# `plugins/db`

## Role

- `plugins/db` is the Tauri transport and app-bootstrap layer for database access.
- It opens the app database, runs app migrations, wires Tauri commands to the reusable live-query runtime, and exposes a minimal JS API.
- This plugin is intentionally thin: it adapts app/runtime concerns downward and exposes transport concerns upward.

## TypeScript Usage

- Use `execute(sql, params?)` for one-shot reads or writes.
- `execute(...)` is the generic query path and returns named object rows.
- `executeProxy(sql, params, method)` is the Drizzle-only path and returns sqlite-proxy positional rows.
- Use `subscribe(sql, params, { onData, onError })` for live query updates.
- Build React hooks such as `useLiveQuery` in app code on top of `subscribe`; do not ship them from this plugin.

## This Folder Owns

- Opening `app_data_dir()/app.db`.
- Running `hypr_db_app::migrate` during plugin setup.
- Tauri command definitions and state management for `execute`, `subscribe`, and `unsubscribe`.
- Adapting `db-live-query::QueryEventSink` onto Tauri `Channel<QueryEvent>`.
- The generated TypeScript bindings and the minimal JS wrapper API.

## This Folder Does Not Own

- SQLite hook installation, pool setup, or cloudsync internals.
- Dependency extraction, watch indexing, rerun policy, or stale-subscriber cleanup.
- App-level React hooks, caching policy, or domain-specific query helpers.
- Schema design beyond invoking the existing Rust migration path.

## Invariants

- Keep schema creation, migrations, and DB initialization in Rust.
- Keep app-specific DB bootstrap here: open `app_data_dir()/app.db` and run `hypr_db_app::migrate`.
- This plugin should only expose query execution and live subscription primitives to TypeScript.
- The JS API should stay low-level and predictable rather than growing app-specific convenience methods.
- Queries should target the app database managed at `app_data_dir()/app.db`.
- Keep the row-shape split explicit: `execute(...)` returns named rows, `executeProxy(...)` returns positional proxy rows.

## Dependency Direction

- This plugin may depend on `db-core2` and `db-live-query`.
- App code may depend on this plugin's JS bindings.
- Lower layers must not depend back on this plugin.

## Design Notes

- Live-query orchestration, dependency analysis, rerun policy, and stale subscriber cleanup belong below this plugin in `db-live-query`.
- If the app needs richer query abstractions, put them in app code or a separate shared package instead of expanding the plugin API surface.

## Test Ownership

- Put tests here when the behavior is about app bootstrap, migration invocation during setup, Tauri command wiring, channel adaptation, generated bindings, or the minimal JS API contract.
- Keep plugin tests thin. Prefer adapter smoke tests over re-testing live-query invalidation behavior that already belongs in `db-live-query`.
- Do not add table-targeting, rerun-policy, or stale-subscriber behavior tests here unless the plugin layer itself changes that behavior.

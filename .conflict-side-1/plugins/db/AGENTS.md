# `plugins/db`

## Use This Plugin For

- Desktop Tauri transport over the app database: bootstrap, one-shot execution, live-query channels, and startup legacy import.

## Put Changes Elsewhere When

- Schema, migration contents, and table helpers belong in `db-app`.
- Open policy, hooks, or CloudSync internals belong in `db-core` / `db-change`.
- Live-query semantics belong in `db-reactive`.
- App-facing hooks, caches, and domain query helpers belong in `apps/desktop`.

## Hard Rules

- Rust owns database opening, migration, and initialization. TypeScript stays a thin command wrapper.
- Keep `execute` and `executeProxy` separate on purpose: named object rows for app SQL, positional rows for Drizzle proxy consumers.
- `QueryEvent` shape and `js/bindings.gen.ts` are ABI. If Rust types change, regenerate bindings; do not hand-edit generated TS.
- `subscribe()` may legitimately return `NonReactive`. The current contract is "warn and keep going," not "throw."
- Startup legacy import must stay idempotent and non-destructive. Existing SQLite rows win over old JSON files by design.
- This plugin is transport-only. Do not add app-specific state, caching, or domain workflows here.
- Every successful subscription needs a matching `unsubscribe`, and the JS wrapper should detach the channel handler before sending it.

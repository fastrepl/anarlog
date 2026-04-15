# `db-core2`

## Role

- `db-core2` is the database substrate layer.
- It owns `Db3`/`DbPool`, SQLite open options, pool lifecycle, storage-recreation primitives, and per-connection SQLite wiring.
- Raw SQLite hook integration belongs here, including `sqlite3_update_hook`.
- Cloudsync integration also belongs here because it is part of how the database is opened and managed, not how queries are exposed to the app.
- Higher layers should consume `Db3`/`DbPool` and raw table-change events from here instead of reimplementing pool setup.

## This Crate Owns

- Opening local and in-memory SQLite databases.
- Applying low-level SQLite pragmas and connection policy.
- Installing per-connection hooks in `SqlitePoolOptions::after_connect`.
- Exposing best-effort table-level mutation notifications for pooled writes.
- Database recreation primitives that upper layers may invoke when their policy requests it.
- Connection-scoped CloudSync helpers that must run on one checked-out executor.
- Keeping `DbPool` ergonomic as an `sqlx::SqlitePool` wrapper.

## This Crate Does Not Own

- App-specific database paths or bootstrap decisions such as `app_data_dir()/app.db`.
- Schema definitions or migrations themselves.
- SQL dependency analysis or `EXPLAIN QUERY PLAN` usage.
- Subscription registries, watcher indexes, rerun policy, stale-subscriber cleanup, or UI delivery.
- Tauri commands, JS bindings, React hooks, or app-facing query APIs.

## Invariants

- Per-connection SQLite setup must happen in `SqlitePoolOptions::after_connect`.
- Do not install `sqlite3_update_hook` from plugin code or from one borrowed connection; that breaks once the pool has more than one physical connection.
- Table-change notifications are best-effort signals for writes observed through this crate's pooled SQLite connections only.
- Change events are table-level, not row-level or predicate-level.
- `DbPool` must continue to `Deref`/`AsRef` to `SqlitePool` so existing SQL callers stay ergonomic.
- App code may supply a migration callback, but the crate must stay schema-agnostic.
- CloudSync operations that require executor affinity should be wrapped here so upper layers do not call `hypr_cloudsync` directly.
- Reactive support must stay additive to normal database usage; callers that do not subscribe should see ordinary open/query behavior.

## Dependency Direction

- Lower-level dependencies such as `sqlx`, SQLite FFI, and cloudsync are allowed here.
- `db-live-query` may depend on this crate.
- `plugins/db` may depend on this crate.
- This crate must not depend on Tauri, transport types, or app query orchestration.

## Design Notes

- If a future change depends on raw SQLite FFI, keep the FFI boundary centralized here.
- If the pool policy changes between single-connection and multi-connection, the hook path should remain correct without upper-layer changes.
- In-memory databases should still be treated carefully; `max_connections(1)` is the safe default unless shared-memory behavior is explicitly intended.
- If code needs to answer "what tables does this SQL depend on?" or "which subscribers should rerun?", it belongs above this crate.

## Test Ownership

- Put tests here when the behavior is about database opening, connection policy, migration failure handling, cloudsync wiring, or raw table-change hook behavior.
- Prefer temp-database integration tests here over higher-level plugin tests when verifying pooled connection semantics.
- Do not test subscription reruns, dependency extraction, transport delivery, or Tauri command behavior here.

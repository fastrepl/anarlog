# `db-watch`

## Role

- `db-watch` is the pure invalidation-index crate.
- It maps `watch id -> referenced tables` and `changed tables -> affected watch ids`.
- It exists so higher layers can reuse table-level targeting logic without pulling in SQL, SQLite, async runtime, or transport code.

## This Crate Owns

- `TableDeps` and similar pure data structures for table-to-watch indexing.
- Registration, lookup, and unregister behavior for watch ids.
- Dedupe and set semantics for affected-watch calculation.

## This Crate Does Not Own

- A database handle or SQLite connection logic.
- SQL parsing, planner inspection, or dependency extraction.
- Async tasks, channels, background workers, or fan-out loops.
- Subscription metadata beyond the table index itself.
- Tauri state, plugin code, JS bindings, or app-specific runtime behavior.

## Invariants

- Keep this crate `unsafe`-free.
- Keep this crate dependency-light and transport-agnostic.
- `TableDeps` is table-granular only. It does not attempt row-level or predicate-level invalidation.
- Empty table sets are allowed and should behave as "never matches".
- `affected(...)` should remain deduped even if the same table name appears multiple times in the input.
- This crate should remain easy to unit test in isolation.

## Dependency Direction

- `db-live-query` may depend on this crate.
- `plugins/db` should not need to depend on this crate directly.
- This crate must not depend on `db-core2`, Tauri, or app crates.

## Design Notes

- If something requires SQL parsing or `EXPLAIN QUERY PLAN`, it belongs in `db-live-query`.
- If something requires SQLite hooks or pool setup, it belongs in `db-core2`.
- If something requires query reruns, subscriber cleanup, or event delivery, it belongs in `db-live-query` or a transport adapter above it.
- Prefer adding tests here when expanding pure targeting behavior; this is the safest place to prove invalidation semantics without runtime coupling.

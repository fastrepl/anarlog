# `db-live-query`

## Role

- `db-live-query` is the reusable live-query service layer.
- It executes SQL, analyzes query dependencies, tracks subscriptions, reruns affected queries, and serializes rows for sinks.
- It consumes raw table-change signals from `db-core2` and maintains its own watch indexing (`watch.rs`).

## This Crate Owns

- `DbRuntime` and the background dispatcher that reacts to table changes.
- Dependency analysis via `extract_tables(...)`.
- Subscription registration and unregistration.
- Distinguishing reactive subscriptions from non-reactive ones.
- Query rerun policy after table-level invalidation.
- Removing stale subscribers when sink delivery fails.
- Converting SQL rows into JSON values for transport-neutral delivery.

## This Crate Does Not Own

- SQLite pool creation, raw SQLite hooks, or database open policy.
- App-specific database bootstrap, file paths, or migrations.
- Tauri commands, Tauri channels, plugin state, or TypeScript bindings.
- React hooks or any UI-facing subscription helpers.
- Row-level or predicate-level invalidation.

## Invariants

- Keep this crate transport-agnostic. No Tauri commands, plugin state, or TypeScript bindings.
- `extract_tables(...)` is the single dependency-analysis path for live-query targeting unless this crate is intentionally redesigned.
- Subscription state must represent dependency analysis explicitly: `Reactive { tables }` or `NonReactive { reason }`.
- Non-reactive subscriptions still deliver the initial query result or error; they simply do not auto-refresh.
- Reruns remain table-granular only.
- Stale subscribers must be removed when event delivery fails.
- The sink trait should stay generic enough that transport adapters can remain thin.

## Dependency Direction

- This crate may depend on `db-core2`.
- `plugins/db` may depend on this crate.
- This crate must not depend on Tauri or app-specific UI/runtime layers.

## Design Notes

- Query execution policy, parameter binding, row serialization, and rerun coalescing belong here.
- Raw SQLite hook installation and pooled connection setup belong below this crate in `db-core2`.
- Tauri channels, app bootstrap, and JS-facing event types belong above this crate in `plugins/db`.
- If this layer ever becomes app-specific, it should be split again rather than letting transport or product logic leak inward.

## Test Ownership

- Put tests here when the behavior is about dependency extraction, reactive vs non-reactive classification, rerun targeting, unsubscribe semantics, stale-sink cleanup, or JSON row serialization.
- These tests may use a real temp database plus a fake sink, but they should not depend on Tauri transport types.
- Higher layers should not duplicate this crate's rerun and invalidation tests unless they are specifically proving adapter integration.

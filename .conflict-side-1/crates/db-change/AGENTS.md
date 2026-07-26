# `db-change`

## Use This Crate For

- Commit-time table invalidation signals sourced from SQLite hooks.
- Monotonic transaction sequencing shared by higher-level reactive consumers.

## Put Changes Elsewhere When

- Pool ownership or connection open policy belongs in `db-core`.
- Dependency analysis and subscription reruns belong in `db-reactive`.
- Row-level or predicate-level invalidation is out of scope here.

## Hard Rules

- Hooks must be installed through `SqlitePoolOptions::after_connect` before connections exist. Attaching later misses writes.
- Signals are best-effort for writes executed through hooked pooled connections only. External writers and unhooked connections are invisible.
- Emit only after commit. Rollback must clear pending state without broadcasting.
- One committed transaction gets one monotonically increasing `seq`; every table event flushed from that transaction shares it.
- Multiple writes to the same table inside one transaction coalesce. `TableChangeKind` is last-write metadata, not a full mutation history.
- Consumers should treat `table + seq` as the invalidation signal. Do not build correctness on the exact `Insert`/`Update`/`Delete` mix.
- Broadcast lag is expected. Keep producers cheap and let consumers decide how to recover from `Lagged`.
- Hook callbacks run on the SQLite path. Do not add async work, blocking I/O, or expensive analysis there.

# `db-reactive`

## Use This Crate For

- Transport-agnostic live queries over `hypr_db_core::Db`.
- Conservative dependency analysis, subscription lifecycle, rerun targeting, and sink delivery.

## Put Changes Elsewhere When

- Pool creation, SQLite hook installation, and open policy belong in `db-core` / `db-change`.
- One-shot SQL execution belongs in `db-execute`.
- Tauri channels, UniFFI listeners, and React hooks belong in transport or app layers.

## Hard Rules

- Reactivity is conservative. If any dependency cannot be resolved or is unsupported, mark the whole subscription `NonReactive`; never return partial target sets.
- Dependency tracking uses canonical targets, not raw table names. New virtual-table support must update both query resolution and raw-table/shadow-table canonicalization.
- The first sink event is always the initial result or initial error. Refresh delivery must never overtake it.
- Writes during subscription setup are handled by seq bookkeeping. Preserve the baseline-seq and catch-up-refresh flow so init does not miss committed changes.
- `unsubscribe()` is a hard delivery barrier. Once it resolves, no more sink callbacks may occur for that subscription.
- Sink failures remove only the failing subscription. One dead transport must not poison the runtime.
- Lagged change receivers or schema-catalog misses degrade by rerunning broadly. Preserve correctness first; precision is optional.
- Reactivity currently depends on `EXPLAIN QUERY PLAN`. Ordinary tables, resolvable views, and supported FTS5 virtual/shadow tables are reactive; unsupported virtual tables must stay explicitly non-reactive.
- The dispatcher intentionally coalesces bursts with a short delay. Treat latency/load changes here as behavioral changes, not refactors.

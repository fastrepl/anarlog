# `db-app`

## Use This Crate For

- The app-owned SQLite schema shared by desktop and mobile.
- Migration manifest (`APP_MIGRATION_STEPS`) and CloudSync table policy.
- Thin SQL row/upsert types and CRUD helpers for app tables.

## Put Changes Elsewhere When

- Pool opening, pragmas, SQLite hooks, or CloudSync runtime wiring belong in `db-core`.
- Migration execution semantics belong in `db-migrate`.
- One-shot transport or live-query transport belongs in `db-execute`, `plugins/db`, or `mobile-bridge`.
- Reactive invalidation logic belongs in `db-reactive`.

## Hard Rules

- Treat migration ids and shipped SQL as append-only. Add a new step instead of mutating an existing one.
- Migration scope is explicit. If DDL touches an enabled CloudSync table, declare `CloudsyncAlter`; never rely on SQL text inspection.
- If a table may ever be synced, make the original DDL CloudSync-safe: one `TEXT NOT NULL` primary key and defaults on every non-PK `NOT NULL` column.
- `cloudsync_table_registry()` is policy, not discovery. Adding or enabling a table here is a deliberate product/runtime decision.
- Desktop and mobile both run `hypr_db_app::schema()`. Schema drift must be fixed here, not in transport layers.
- JSON-shaped fields stay as opaque `TEXT`. Parsing and legacy-shape normalization belong in adapters/importers, not this crate.
- `events.calendar_id` is intentionally not a foreign key today. Adding FK or cascade behavior changes import, delete, and sync semantics; audit all three first.
- Keep helper behavior stable: upserts preserve `created_at`, bump `updated_at`, and list queries stay deterministic unless you intentionally want downstream churn.
- `insert_*_if_missing` is import-only. Normal write paths should use the upsert helpers.

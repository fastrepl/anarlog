# `db-migrate`

## Purpose

`db-migrate` owns app-database migration execution.

- Input: a checked-open `Db3` from `db-core2` plus a schema manifest from a schema crate such as `db-app`
- Output: schema changes applied and recorded in `_sqlx_migrations`

This crate exists to keep:

- `db-core2` focused on database opening, pooling, and SQLite/CloudSync primitives
- schema crates focused on migration manifests and table meaning
- CloudSync-sensitive migration mechanics enforced in one place

## Model

Treat this crate as a narrow port of `sqlx` migration behavior, not as a custom migration system.

It should preserve the usual `sqlx` SQLite semantics:

- `_sqlx_migrations` history table
- ordered apply
- checksum validation
- dirty-version detection
- idempotent re-run behavior

The only intentional divergence is explicit per-step scope:

- `Plain`
- `CloudsyncAlter { table_name }`

## Why Not Just Use `sqlx::Migrator`

Built-in `sqlx` migrator logic is close, but it does not expose a hook for:

1. `cloudsync_begin_alter_on(conn, table)`
2. run DDL on that same `conn`
3. `cloudsync_commit_alter_on(conn, table)`

For ordinary SQLite migrations, pool-level execution is fine.
For CloudSync alter steps, it is not.

The invariant is:

```text
same checked-out connection:
  begin_alter
  DDL
  commit_alter
```

Not:

```text
pool:
  begin_alter -> conn A
  DDL         -> conn B
  commit      -> conn C
```

`max_connections = 1` is not a real substitute. The requirement is explicit ownership of one connection across the whole alter protocol, not merely "the pool only has one connection available right now."

## API

```rust
pub async fn migrate(db: &Db3, schema: DbSchema) -> Result<(), MigrateError>
```

Callers must open the database first. This crate does not own connection setup or storage configuration.

## Ownership Boundary

This crate owns:

- migration orchestration
- translation from `MigrationStep` to `sqlx::migrate::Migration`
- validation of step ids, duplicate versions, and CloudSync-target eligibility
- execution semantics for `Plain` vs `CloudsyncAlter`
- `_sqlx_migrations` bookkeeping

This crate does not own:

- pool creation or database opening
- CloudSync extension loading or network/runtime setup
- app table definitions, row types, or query APIs
- migration SQL contents
- inference of whether a step "looks like" a CloudSync alter

If a change is about schema meaning, it probably belongs in the schema crate.
If a change is about how migrations are executed, it probably belongs here.

## Rules

- Keep behavior as close to upstream `sqlx` as possible.
- Add divergence only when CloudSync or connection-control requirements force it.
- Make migration scope explicit in the manifest; do not infer it from SQL text.
- For `CloudsyncAlter`, use connection-scoped helpers from `db-core2`, never pool-level wrappers.
- When CloudSync is disabled, the same `CloudsyncAlter` step should fall back to normal SQLite execution so local and synced schemas stay aligned.
- Prefer a small, auditable port over a growing custom framework.

## Testing

Tests here should cover migration execution semantics, not app behavior.

Keep coverage focused on:

- plain migration parity with `sqlx` expectations
- idempotent re-runs
- checksum/version validation failures
- manifest validation failures
- CloudSync alter behavior on one checked-out connection
- CloudSync-disabled fallback for `CloudsyncAlter`

Do not put app-specific query or domain tests here.

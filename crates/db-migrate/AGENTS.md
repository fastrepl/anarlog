# `db-migrate`

## Role

- `db-migrate` is the app-database migration engine.
- It owns migration orchestration, migration history bookkeeping, and schema change execution.
- It exists to keep schema declaration crates such as `db-app` focused on tables, types, ops, and migration manifests, while keeping CloudSync-sensitive migration mechanics in a core-adjacent layer.

## Why This Crate Exists (why not sqlx's builtin migrator)

sqlx's `Migrator::run()` applies SQL in a transaction and records it — but gives no hook to run per-connection setup/teardown around each DDL statement. CloudSync alter requires `cloudsync_begin_alter_on(conn, table)` → DDL → `cloudsync_commit_alter_on(conn, table)` **on the same connection**. sqlx's `apply()` owns the connection internally, so there's no way to inject these calls.

The migration runner here reimplements the subset of sqlx's migrator that we need (checksum validation via SHA-384, history table, idempotent apply) while adding the CloudSync scope semantics. See `sqlx-core/src/migrate/` and `sqlx-sqlite/src/migrate.rs` in the sqlx repo for the upstream implementation this is based on.

Other reasons this crate exists:
- `db-core2` is schema-agnostic substrate. It should open databases, manage pools, and expose CloudSync/SQLite primitives, but it should not know app schema history.
- `db-app` is schema declaration. It should define the CloudSync table registry and migration steps, but it should not own migration execution.
- CloudSync-backed schema changes introduce operational constraints that are stronger than ordinary SQLite migrations, so the runner needs to enforce them centrally instead of leaving each caller to remember them.

## API

The crate exposes a single entry point:

```rust
pub async fn migrate(db: &Db3, schema: DbSchema) -> Result<(), MigrateError>
```

Callers are responsible for opening the database via `db-core2` first, then passing it here to run migrations. This keeps storage/connection concerns in `db-core2` and migration concerns here.

## This Crate Owns

- The `_char_migrations` history table for post-baseline migration steps.
- Execution of migration steps with explicit scope:
  - `Plain`
  - `CloudsyncAlter { table_name }`
- Validation that CloudSync alter steps only target tables declared as synced by the schema crate.

## This Crate Does Not Own

- Database opening, connection pooling, or storage configuration (that's `db-core2`).
- App table definitions, row types, or query/ops functions.
- The set of synced tables for a given app schema.
- Migration `.sql` files themselves (those live in the schema crate, embedded via `include_str!`).
- Raw SQLite/CloudSync connection setup and same-connection CloudSync alter helpers. That belongs in `db-core2`.

## CloudSync Constraints

Treat CloudSync-backed schema changes as a different class of migration from normal SQLite DDL.

- For synced-table schema changes, the runner must use:
  1. `db-core2`'s connection-scoped `cloudsync_begin_alter_on(...)`
  2. run the DDL on the same checked-out connection
  3. `db-core2`'s connection-scoped `cloudsync_commit_alter_on(...)`
- Do not run `begin_alter` / DDL / `commit_alter` through a pool-level API that may hop connections.
- Do not hide CloudSync alter behavior behind SQL parsing or table-name inference. Migration steps must declare CloudSync scope explicitly.
- When CloudSync is disabled at open time, the same schema step may run without the alter wrapper so local and synced schemas remain structurally aligned.

## Design Rules

- Keep the runner generic over schema providers. Schema crates should pass:
  - migration step manifest (using `include_str!` for SQL, checksums computed at runtime via SHA-384)
  - CloudSync table validator
- Prefer explicit step metadata over "magic" inspection.
- Add new migration policy here only when it is about migration execution semantics, not about schema meaning.
- If a future change only affects one app's schema contents, it probably belongs in that schema crate, not here.

## Testing Ownership

- Put tests here when behavior is about:
  - migration history bookkeeping
  - CloudSync alter-step validation
  - migration orchestration
- Do not test app-specific query behavior here. That belongs in the schema crate.

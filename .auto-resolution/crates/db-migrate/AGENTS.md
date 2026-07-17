# `db-migrate`

## Role

App-database migration execution. Narrow port of `sqlx` migration behavior with one intentional divergence: explicit per-step scope (`Plain` vs `CloudsyncAlter`) to enforce connection affinity during CloudSync DDL.

## Owns

- Migration orchestration and `_sqlx_migrations` bookkeeping.
- `MigrationStep` → `sqlx::migrate::Migration` translation.
- Validation of step ids, duplicate versions, CloudSync-target eligibility.
- Execution semantics for `Plain` vs `CloudsyncAlter { table_name }`.

## Does Not Own

- Pool creation or database opening.
- CloudSync extension loading or network setup.
- App table definitions, row types, query APIs, migration SQL contents.
- Inference of whether a step "looks like" a CloudSync alter.

## Invariants

- `CloudsyncAlter` requires `begin_alter` → DDL → `commit_alter` on the same checked-out connection. Pool-level execution is not acceptable.
- When CloudSync is disabled, `CloudsyncAlter` falls back to normal SQLite execution.
- Preserves `sqlx` semantics: ordered apply, checksum validation, dirty-version detection, idempotent re-run.
- Migration scope is explicit in the manifest; never inferred from SQL text.

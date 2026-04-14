---
name: migrate-to-sqlite
description: Migrate a TinyBase table to SQLite. Use when asked to move a data domain (e.g. templates, chat shortcuts, vocabs) from the TinyBase store to the app SQLite database.
---

## Architecture

- **Schema source of truth:** Rust migration in `crates/db-app/migrations/`
- **Drizzle mirror:** `packages/db/src/schema.ts` (typed TS query interface, not schema management)
- **Reads (reactive):** `useDrizzleLiveQuery` — calls `.toSQL()` on a Drizzle query, feeds `{sql, params}` to `useLiveQuery` which uses `subscribe()` from `@hypr/plugin-db`
- **Reads (imperative):** `db.select()...` through the Drizzle sqlite-proxy driver
- **Writes:** `db.insert()`, `db.update()`, `db.delete()` through the Drizzle sqlite-proxy driver, wrapped in `useMutation` from tanstack-query
- **Reactivity loop:** write via `execute` → SQLite change → Rust `db-live-query` notifies subscribers → `useLiveQuery` fires `onData` → React re-renders. No manual invalidation needed.

## Steps

### 1. Rust migration

Add a new timestamped `.sql` file in `crates/db-app/migrations/`. Convention: `YYYYMMDDHHMMSS_name.sql`.

Do NOT include `user_id` columns — it was a TinyBase-era pattern with a hardcoded default. It will be redesigned when multi-device/team support lands.

### 2. Rust ops (optional but recommended)

Add `<domain>_types.rs` and `<domain>_ops.rs` in `crates/db-app/src/` with typed `sqlx::FromRow` structs and CRUD functions. Export from `lib.rs`. These are used by other Rust code and legacy import; the TS side uses Drizzle instead.

### 3. Legacy data import

If the domain had a TinyBase JSON persister file (e.g. `templates.json`), add an import function in `plugins/db/src/migrate.rs` that reads the old file and upserts rows. Call it from `plugins/db/src/runtime.rs` during startup. Guard with an "already imported" check (e.g. table non-empty).

### 4. Drizzle schema

Add the table definition to `packages/db/src/schema.ts` mirroring the migration. Use `{ mode: "json" }` for JSON text columns, `{ mode: "boolean" }` for integer boolean columns. Re-export from `packages/db/src/index.ts` if adding new operator re-exports.

### 5. TS consumer migration

Replace raw TinyBase reads/writes with:
- `useDrizzleLiveQuery(db.select()...)` for reactive reads
- `db.select()...` for imperative reads (returns parsed objects via proxy driver)
- `db.insert()`, `db.update()`, `db.delete()` for writes, wrapped in `useMutation`

Live query results come from Rust `subscribe` as raw objects (not through Drizzle driver), so `mapRows` must still handle JSON parsing for JSON columns.

### 6. Remove TinyBase artifacts

- Table definition from `packages/store/src/tinybase.ts`
- Query definitions from `store/tinybase/store/main.ts` (both `QUERIES` object and `_QueryResultRows` type)
- Persister files (e.g. `store/tinybase/persister/<domain>/`)
- Persister registration from `store/tinybase/store/persisters.ts`
- Hooks from `store/tinybase/hooks/` if they existed
- Associated tests and test wrapper setup

### 7. Verify

- `cargo check` and `cargo test -p db-app -p tauri-plugin-db`
- `pnpm -F @hypr/desktop typecheck`
- `pnpm -F @hypr/desktop test`
- `pnpm exec dprint fmt`

---
name: migrate-to-sqlite
description: Migrate a TinyBase table to SQLite. Use when asked to move a data domain (e.g. templates, vocabs) from the TinyBase store to the app SQLite database.
---

## Status

Keep up to date as each PR lands. Outer box = fully done across both
phases. Sub-bullets track sub-states where relevant.

This migration targets the TinyBase `main` store only. The separate
TinyBase `settings` store is out of scope unless the plan is explicitly
expanded to include it.

- [x] `templates` — already Drizzle, no Phase 0 needed
- [ ] `calendars`
  - [x] Phase 0 reads (PR 2: `useCalendar`, `useEnabledCalendars`)
  - [ ] Phase 0 writes — `services/calendar/ctx.ts` has a cross-domain
        calendars+events transaction; lands with events PR
  - [ ] Phase 1 — Rust migration + ops exist
- [ ] `events`
  - [ ] Phase 0
  - [ ] Phase 1 — Rust migration + ops exist
- [ ] `sessions`
- [ ] `transcripts`
- [ ] `humans`
- [ ] `organizations`
- [ ] `enhanced_notes`
  - [x] Phase 0 reads — `session/hooks/useEnhancedNotes.ts`
  - [ ] Phase 0 writes
  - [ ] Phase 1
- [ ] `mapping_session_participant`
- [ ] `mapping_tag_session`
- [ ] `mapping_mention`
- [ ] `tags`
- [ ] `chat_groups`
- [ ] `chat_messages`
  - [x] Phase 0 writes (partial) — `chat/store/*`
  - [ ] Phase 0 reads
  - [ ] Phase 1
- [ ] `tasks`
- [ ] `memories`
  - [x] Phase 0 writes — `settings/memory/custom-vocabulary.tsx`
  - [ ] Phase 0 reads
  - [ ] Phase 1
- [ ] `daily_notes`

## Strategy

Two-phase, per-domain migration. Each phase is many small PRs.

### Phase 0 — Decouple consumers

Before any storage swap, move every TinyBase call behind a domain hook
living in `apps/desktop/src/<domain>/hooks.ts` (or `<domain>/hooks/*`).
Hook return shapes are plain TypeScript; no TinyBase types leak out.
Consumer code stops importing `~/store/tinybase/store/main`.

Scope note: this applies to consumers of
`~/store/tinybase/store/main` and the legacy `~/store/tinybase/hooks`
barrel. Do not start wrapping `settings.UI.*` usage behind new hooks as
part of this effort; `~/store/tinybase/store/settings` is intentionally
out of scope for this migration.

Why: one storage-swap PR per domain touches 1 file (the hook module),
not 20–50 consumer files.

Enforced by `hypr/no-raw-tinybase` in `eslint-plugin-hypr.mjs`.
`.oxlintrc.json` keeps a `TINYBASE_MIGRATION_PENDING` override that
shrinks as each domain is cleaned. CI gates this via
`.github/workflows/lint.yaml`.

### Phase 1 — Swap storage per domain

1. Rust migration + ops + Drizzle schema (steps below).
2. Flip writes in the hook module: `db.insert()/update()/delete()`.
3. **Shadow-hydrate** TinyBase from SQLite so read hooks that haven't
   moved yet keep working. A small adapter subscribes to `db-live-query`
   and mirrors rows into the in-memory TinyBase store. One-way only:
   SQLite is the source of truth.
4. Swap read hooks to `useDrizzleLiveQuery` one at a time. Consumers
   untouched because hook signatures are stable.
5. Once no TinyBase consumers remain for the domain, remove the shadow
   adapter, the TinyBase schema entries, and the persister.

Skip the shadow bridge only for leaf-clean domains (no cross-table
indexes/queries into or out of the domain, and <10 consumer sites).

## Architecture

- **Schema source of truth:** Rust migration in `crates/db-app/migrations/`
- **Drizzle mirror:** `packages/db/src/schema.ts` (typed TS query interface, not schema management)
- **Reads (reactive):** `useDrizzleLiveQuery` — calls `.toSQL()` on a Drizzle query, feeds `{sql, params}` to the underlying `useLiveQuery` which uses `subscribe()` from `@hypr/plugin-db`
- **Reads (imperative):** `db.select()...` through the Drizzle sqlite-proxy driver
- **Writes:** `db.insert()`, `db.update()`, `db.delete()` through the Drizzle sqlite-proxy driver, wrapped in `useMutation` from tanstack-query
- **Reactivity loop:** write via `execute` → SQLite change → Rust `db-live-query` notifies subscribers → `useLiveQuery` fires `onData` → React re-renders. No manual invalidation needed.

### Package layers

The DB stack uses a factory/DI pattern across four packages:

1. `@hypr/db-runtime` (`packages/db-runtime/`) — type contracts only: `LiveQueryClient`, `DrizzleProxyClient`, shared row/query types.
2. `@hypr/db` (`packages/db/`) — Drizzle schema (`schema.ts`) + `createDb(client)` factory using `drizzle-orm/sqlite-proxy`. Re-exports Drizzle operators (`eq`, `and`, `sql`, etc.).
3. `@hypr/db-tauri` (`packages/db-tauri/`) — Tauri-specific client that binds `execute`/`executeProxy`/`subscribe` from `@hypr/plugin-db` to the `db-runtime` types.
4. `@hypr/db-react` (`packages/db-react/`) — `createUseLiveQuery(client)` and `createUseDrizzleLiveQuery(client)` factories.

These are wired together in `apps/desktop/src/db/index.ts`, which exports `db`, `useLiveQuery`, and `useDrizzleLiveQuery`. **Consumer code imports from `~/db`, not directly from the packages.**

## Per-domain steps (Phase 1)

Assumes Phase 0 already landed for this domain — consumers go through
`<domain>/hooks.ts`, not raw `UI.*`.

### 1. Rust migration

Add a new timestamped `.sql` file in `crates/db-app/migrations/`. Convention: `YYYYMMDDHHMMSS_name.sql`.

Do NOT include `user_id` columns — it was a TinyBase-era pattern with a hardcoded default. It will be redesigned when multi-device/team support lands.

### 2. Rust ops (optional but recommended)

Add `<domain>_types.rs` and `<domain>_ops.rs` in `crates/db-app/src/` with typed `sqlx::FromRow` structs and CRUD functions. Export from `lib.rs`. These are used by other Rust code and legacy import; the TS side uses Drizzle instead.

### 3. Legacy data import

If the domain had a TinyBase JSON persister file (e.g. `templates.json`), add an import function in `plugins/db/src/migrate.rs` that reads the old file and upserts rows. Call it from `plugins/db/src/runtime.rs` during startup. Guard with an "already imported" check (e.g. table non-empty).

### 4. Drizzle schema

Add the table definition to `packages/db/src/schema.ts` mirroring the migration. Use `{ mode: "json" }` for JSON text columns, `{ mode: "boolean" }` for integer boolean columns. Re-export from `packages/db/src/index.ts` if adding new operator re-exports.

### 5. Swap hook internals

Replace the hook module's TinyBase calls with Drizzle. Hook signatures
stay the same, so consumer code doesn't change.

- `useDrizzleLiveQuery(db.select()...)` for reactive reads
- `db.select()...` for imperative reads (returns parsed objects via proxy driver)
- `db.insert()`, `db.update()`, `db.delete()` for writes, wrapped in `useMutation`

Import `db` and `useDrizzleLiveQuery` from `~/db`, and schema tables/operators from `@hypr/db`.

Live query results come from Rust `subscribe` as raw objects (not through the Drizzle driver), so `mapRows` must handle two things:

- **JSON parsing** for JSON text columns (e.g. `sections_json`, `targets_json`).
- **snake_case → camelCase mapping.** Live rows use the raw SQLite column names (`pin_order`, `targets_json`), while Drizzle's `$inferSelect` uses camelCase (`pinOrder`, `targetsJson`). Define a separate `<Domain>LiveRow` type with snake_case keys for `mapRows`, distinct from the Drizzle inferred type. See `TemplateLiveRow` in `apps/desktop/src/templates/queries.ts` for the pattern.

### 6. Shadow bridge (skip for leaf-clean domains)

Add a small module that hydrates the TinyBase `<domain>` table from
SQLite on startup and subscribes to `db-live-query` to mirror subsequent
changes. One-way: SQLite → TinyBase. Retire once all consumers have
swapped.

### 7. Remove TinyBase artifacts

- Table definition from `packages/store/src/tinybase.ts`
- Query definitions from `store/tinybase/store/main.ts` (both `QUERIES` object and `_QueryResultRows` type)
- Persister files (e.g. `store/tinybase/persister/<domain>/`)
- Persister registration from `store/tinybase/store/persisters.ts`
- Hooks from `store/tinybase/hooks/` if they existed
- Shadow-bridge module if one was added
- Associated tests and test wrapper setup

### 8. Verify

- `cargo check` and `cargo test -p db-app -p tauri-plugin-db`
- `pnpm -F @hypr/desktop typecheck`
- `pnpm -F @hypr/desktop test`
- `npx oxlint --quiet apps/desktop/src/` (the `hypr/no-raw-tinybase` CI gate)
- `pnpm exec dprint fmt`

# `packages/db`

## Role

- Owns the Drizzle-facing TypeScript adapter for the app database schema.
- Exposes `createDb(...)` and re-exports the Drizzle schema/helpers used by app code.

## Invariants

- This package is a thin Drizzle adapter, not a generic SQL transport layer.
- `createDb(...)` must talk to a transport that implements the Drizzle sqlite-proxy contract directly.
- Drizzle proxy reads and writes use `executeProxy(sql, params, method)`.
- `executeProxy(...)` returns positional rows in SQL select order, as expected by `drizzle-orm/sqlite-proxy`.
- This package must not parse SQL or remap named object rows into positional proxy rows.
- Named object rows remain the responsibility of the generic live-query transport below this layer.

## Dependency Direction

- May depend on `@hypr/db-runtime` for the proxy client contract.
- Must not own Tauri/mobile transport details or database initialization.

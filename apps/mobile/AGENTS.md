# Anarlog Mobile

Expo (SDK 57) app for Anarlog. Expo has changed significantly — read the exact versioned docs at https://docs.expo.dev/versions/v57.0.0/ before writing any code.

## Commands

- Dev: `pnpm -F @hypr/mobile ios` (or `android`)
- Typecheck: `pnpm -F @hypr/mobile typecheck`

## Direction

- Local-first client on the canonical SQLite schema (`crates/db-app`) via the UniFFI `crates/mobile-bridge` transport. The TS side should implement the `@hypr/db-runtime` client contracts (like `packages/db-tauri` does for desktop) once the native module lands.
- Screens currently render mock data from `src/data/sessions.ts`; swap it for live queries when the bridge transport exists.
- UX reference: `design/README.md`.

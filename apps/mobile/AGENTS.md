# Anarlog Mobile

Expo (SDK 57) app for Anarlog. Expo has changed significantly — read the exact versioned docs at https://docs.expo.dev/versions/v57.0.0/ before writing any code.

## Commands

- Dev: `pnpm -F @hypr/mobile ios` (or `android`; scripts load `.env.supabase` via dotenvx)
- Typecheck: `pnpm -F @hypr/mobile typecheck`

## Architecture

- Local-first, Pro-only client on the canonical SQLite schema. `src/db/` is the mobile twin of `packages/db-tauri`: an expo-sqlite transport implementing the `@hypr/db-runtime` `LiveQueryClient`/`TransactionClient` contracts, consumed through `@hypr/db-react`'s `useLiveQuery`.
- `src/db/migrations.ts` is an interim TS mirror of `crates/db-app/migrations` (subset, final shape). The UniFFI `crates/mobile-bridge` will own the schema later; this local DB is disposable — cloud sync repopulates it. Keep every statement semantically identical to desktop (`apps/desktop/src/session/queries.ts` is the reference for session SQL).
- `src/data/` mirrors desktop query semantics: canonical create-session transaction, note docs as ProseMirror JSON with `id == session_id`, `session-audio:<sessionId>` attachment rows.
- `src/auth/` is supabase-js with AsyncStorage plus the desktop browser-handoff flow (`/auth?flow=desktop&scheme=anarlog`). Pro gating uses the same JWT-claims logic as `packages/supabase/src/billing.ts` (ported, since jose does not run on Hermes). No Supabase env → bypass mode (local dev, no gate).
- Recording/import write files under `<documents>/sessions/<sessionId>/audio.<ext>` and catalog them via `src/data/audio-catalog.ts`. No on-device STT models: `src/data/transcribe.ts` posts the audio to the `{API}/stt/listen` batch proxy (desktop-parity transcript rows, `source: batch_transcription`) and flips `transcript_status` to `complete`; failures leave it `processing` with a retry affordance on the note screen.

## Rules

- Local writes never wait on network. Remote side effects are best-effort afterwards.
- Keep schema/SQL parity with desktop; do not invent mobile-only columns or enums.
- UX reference: `design/README.md`.

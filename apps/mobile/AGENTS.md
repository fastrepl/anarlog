# Anarlog Mobile

Expo (SDK 57) app for Anarlog. Expo has changed significantly — read the exact versioned docs at https://docs.expo.dev/versions/v57.0.0/ before writing any code.

## Commands

- Dev: `pnpm -F @anlg/mobile ios` (or `android`; scripts load `.env.supabase` via dotenvx)
- Typecheck: `pnpm -F @anlg/mobile typecheck`

## Architecture

- Local-first client on the canonical SQLite schema. Signed-in users can use local notes and recording on any plan. The shared two-week Pro trial and paid Pro entitlement enable cloud sync and Anarlog-hosted models; users can also configure their own provider keys. `src/db/` uses the UniFFI `crates/mobile-bridge` transport to implement the `@anlg/db-runtime` `LiveQueryClient`/`TransactionClient` contracts, consumed through `@anlg/db-react`'s `useLiveQuery`.
- `crates/db-app` owns the canonical schema and migrations on mobile and desktop. Keep every statement semantically identical to desktop (`apps/desktop/src/session/queries.ts` is the reference for session SQL).
- `src/data/` mirrors desktop query semantics: canonical create-session transaction, note docs as ProseMirror JSON with `id == session_id`, `session-audio:<sessionId>` attachment rows.
- `src/auth/` is supabase-js with AsyncStorage plus the desktop browser-handoff flow (`/auth?flow=desktop&scheme=anarlog`). Pro gating uses the same JWT-claims logic as `packages/supabase/src/billing.ts` (ported, since jose does not run on Hermes). No Supabase env → bypass mode (local dev, no gate).
- Recording/import write files under `<documents>/sessions/<sessionId>/audio.<ext>` and catalog them via `src/data/audio-catalog.ts`. No on-device STT models: `src/data/transcribe.ts` uses the Anarlog batch proxy or the selected BYOK provider. Native BYOK transcription reuses `owhisper-client` adapters through `mobile-bridge`; Custom uses the OpenAI-compatible HTTP endpoint. Recording streams PCM to Anarlog Pro or supported BYOK live models through the same `owhisper-client` adapters as desktop. Live results write canonical `live_capture` transcripts and deltas; imported audio, batch models, and live failures use `batch_transcription` with the corresponding batch model. Both paths mark completed audio as transcribed. Live-only providers keep failed recordings available for retry with another provider.

## Rules

- Local writes never wait on network. Remote side effects are best-effort afterwards.
- Keep schema/SQL parity with desktop; do not invent mobile-only columns or enums.
- Store marketing version lives in `apps/mobile/release-version.json` and is independent of desktop `release-version.json`. Bump it with `node scripts/release-version.mjs --mobile <major.minor.patch>`.
- UX reference: `design/README.md`.

## Keep implementations simple

- Keep `src/app/` for routes and navigation. Put substantial screen workflows beside their feature (for example, `src/note/`), with components owning the state they render. Keep app initialization and providers in the root layout.
- Use queries and mutations for async status and errors; use TanStack Form for editable form values. Derive values instead of mirroring them in state. Keep synchronous guards when async state cannot prevent same-frame taps.
- Give mutually exclusive UI states one representation, such as the currently open sheet. Use a keyed screen boundary to scope drafts and async work to a route's entity.
- Extract code when it separates a responsibility or removes duplication. Avoid generic controllers, wrapper hooks, and interfaces that only rename a single call. Prefer explicit domain functions over configurable frameworks.
- Preserve native input/caret behavior, focus and modal dismissal ordering, accessibility, local-save guarantees, and cancellation/retry semantics during refactors. Add tests for persistence and lifecycle guarantees, not component or file layout.

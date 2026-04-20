# `@hypr/desktop2`

Electron port of the note-taking desktop app. Context on the port and
non-goals is in `README.md`.

## Commands

- Typecheck: `pnpm -F @hypr/desktop2 check`
- Dev: `pnpm turbo run dev -F @hypr/desktop2`
- Build: `pnpm turbo run build -F @hypr/desktop2`
- Bundle: `pnpm turbo run bundle -F @hypr/desktop2`
- Repo format after edits: `pnpm exec dprint fmt`

Run through `turbo` for dev/build/bundle so `@hypr/napi-sdk` (NAPI) rebuilds
upstream. Direct `pnpm -F @hypr/desktop2 <script>` skips the SDK rebuild and
will yield stale bindings.

## Process boundary

Four source roots. Do not cross-wire them.

- `src/main/**` runs in Node for the Electron main process. No React, no DOM.
- `src/preload/**` runs in Node for the preload bridge. No React, no DOM.
- `src/renderer/**` runs in the browser renderer. No `electron`, `fs`, `path`,
  etc.
- `src/shared/**` is pure TypeScript used by main/preload and renderer. IPC
  channel names, DTO types that are not generated, and small snapshot
  reducers live here. Both sides import via relative paths
  (`../shared/...` or `../../shared/...` depending on depth); do not
  copy-paste contents into either side.

The renderer reads `window.hypr`, typed against `HyprElectronApi` in
`src/shared/api.ts`. Do not touch the global in new call sites; import
`hypr` from `~/bridge`.

## Data access

We do not hand-roll per-entity IPC. The contract mirrors `plugins/db` on
the Tauri side:

- `window.hypr.db` implements `DrizzleProxyClient & LiveQueryClient` from
  `@hypr/db-runtime`. Four methods: `execute`, `executeProxy`, `subscribe`,
  plus an unsubscribe returned from `subscribe`.
- `@hypr/db-electron` wraps `window.hypr.db` so `@hypr/db` can plug in via
  `createDb(electronLiveQueryClient)`. Matches `@hypr/db-tauri`.
- Renderer queries through drizzle, not per-entity methods. Use
  `useDrizzleLiveQuery(db.select()...)` from `~/db` for reactive reads,
  `db.insert/update/delete` for writes.
- Rust stays generic: `hypr-napi` only exposes `init`, `execute`,
  `executeProxy`, `subscribe`. Schema + migrations live in `hypr-db-app2`;
  the renderer's drizzle schemas live in `@hypr/db`. The two must stay in
  lockstep.

## IPC contract

- `hyprIpcChannels` is declared once in `src/shared/channels.ts`. Both
  `main.ts` and `preload.cts` import it — do not inline `"hypr:…"`
  strings.
- Generic data channels: `hypr:db:{execute,executeProxy,subscribe,
  unsubscribe}`. Do not add per-entity channels here; add drizzle usage
  in the renderer instead.
- Non-DB Electron surface on `window.hypr`: `openExternal`, `embeddedCli.*`,
  `updater.*`. Their shared types live in `src/shared/embedded-cli.ts`
  and `src/shared/updater.ts`.

## Subscription deltas

- Rust → renderer lifecycle runs through
  `src/main/subscription-manager.ts`. `main.ts` must not hold
  `SubscriptionHandle` values directly.
- Deltas go to the subscribing `WebContents` only via a private channel
  (`hypr:db:subscribe:delta:<uuid>`), never to
  `BrowserWindow.getAllWindows()`. Rationale is in
  `src/main/subscription-architecture.md`.

## Native shell

- Native-shell parity (window chrome, tray, activation policy, …) is the
  Tauri port table in `README.md`. When adding a behavior, map it to the
  Tauri source it replaces in a comment.
- Assets shipped in production come through either `resourcePath(…)` in
  `src/main/paths.ts` + `build.extraResources` in
  `electron-builder.config.ts`, or `embeddedCliPath()` +
  `build.mac.extraFiles` for the packaged `char` CLI. Do not read from
  relative repo paths at runtime.

## Channels

Three channels — `stable`, `nightly`, `staging` — each a **distinct installable
product** with its own `appId`, executable name, deep-link scheme, icon set,
DMG background, and `userData` directory. Shape mirrors
`apps/desktop/src-tauri/tauri.conf.<channel>.json` re-namespaced under
`com.char.*`.

| Channel | `appId` | `productName` | Deep-link scheme | Updater |
| --- | --- | --- | --- | --- |
| `stable` | `com.char.stable` | `Char` | `hyprnote`, `char` | enabled (future) |
| `nightly` | `com.char.nightly` | `Char Nightly` | `hyprnote-nightly` | enabled (future) |
| `staging` | `com.char.staging` | `Char Staging` | `hyprnote-staging` | disabled |

- Channel selection: `HYPR_CHANNEL` env var at build/package time. Default
  locally is `staging`.
- Channel is stamped into the packaged `package.json` as `hyprChannel` via
  `electron-builder.config.ts#extraMetadata`; `src/main/channel.ts` reads
  it back at runtime.
- `main.ts` calls `app.setName` / `app.setAppUserModelId` /
  `app.setPath("userData", …)` with the channel's `appId` BEFORE
  `app.whenReady()`. This makes channel installs side-by-side on one machine.
- `UPDATER_ENABLED` is `app.isPackaged && CHANNEL !== "staging"`; consume it
  from `channel.ts` when wiring `electron-updater`.

## Embedded CLI

Parity with Tauri's `apps/desktop/src-tauri/src/embedded_cli.rs` (installs
`/usr/local/bin/<cmd>` as a symlink into the app bundle, macOS-only):

- Binary built by `cargo xtask prepare-desktop2-binaries` into
  `apps/desktop2/binaries/char-cli-<triple>`. Use
  `pnpm -F @hypr/desktop2 build:embedded-cli` locally; CI sets
  `TAURI_ENV_TARGET_TRIPLE` per matrix arch; local runs fall back to rustc's
  host triple.
- Packaging: `electron-builder.config.ts#mac.extraFiles` copies the
  arch-matched binary to `<App>.app/Contents/MacOS/char-cli`.
- Runtime resolver: `src/main/paths.ts::embeddedCliPath()`.
- Install/uninstall: `src/main/embedded-cli.ts`. Direct `fs.symlink`
  first, AppleScript fallback for unwritable `/usr/local/bin`.
- IPC: `hypr.embeddedCli.{check,install,uninstall}` → `EmbeddedCliStatus`.
  UI is `src/renderer/settings/command-line/` (ported from
  `apps/desktop/src/settings/lab/command-line.tsx`).

## Packaging

- Distribution is `electron-builder` (not Forge). Config lives in
  `electron-builder.config.ts` at the app root, not inline in `package.json`.
- The `package.json#build` key is intentionally absent; `pnpm bundle` passes
  `--config electron-builder.config.ts` explicitly.
- Code signing / notarization wiring is the CD workflow's concern
  (`.github/workflows/desktop2_cd.yaml` reuses `./.github/actions/apple_cert`
  and enables notarization via `CI=true && APPLE_ID`).

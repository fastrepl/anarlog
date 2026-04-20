# `@hypr/desktop2`

Electron proof-of-concept for the `sessions` migration slice.
Ships as **Char 2** (`com.hyprnote.desktop2`), using the `pro` icon set from
`apps/desktop/src-tauri/icons/pro/`.

## Source layout

`desktop2` now keeps all runtime code under `src/`:

- `src/main/**` for the Electron main process
- `src/preload/**` for the preload bridge
- `src/renderer/**` for the React/Vite renderer
- `src/shared/**` for shared IPC types, channel names, and reducers

## What this app proves

- Electron main can call Rust through `@hypr/napi-sdk` (`crates/hypr-napi`).
- Reactive updates can stay on the same N-API transport as CRUD, fanned out
  to the right renderer via `src/main/subscription-manager.ts` (private
  per-subscriber channels, not `BrowserWindow.getAllWindows()` broadcasts).
- A thin React shell can edit sessions without depending on the existing Tauri
  plugin graph.
- IPC DTOs are generated from Rust via `specta` (not hand-maintained).
- The **native shell** (frameless window, traffic-light layout, tray, single
  instance, macOS accessory mode) can be reproduced in Electron without pulling
  in any Tauri plugin — see `src/main/window.ts` and
  `src/main/tray.ts`.

## Native shell parity

The goal is not feature parity with `apps/desktop`; it is to prove each OS-level
primitive we currently rely on from Tauri has a known-good Electron shape. What
is wired up today:

| Behavior                        | Tauri source                                    | Electron port                                                 |
| ------------------------------- | ----------------------------------------------- | ------------------------------------------------------------- |
| Hidden title + traffic lights   | `plugins/windows/src/window/v1.rs`              | `titleBarStyle: "hiddenInset"` + `trafficLightPosition`       |
| No decorations on Win/Linux     | `decorations(false)` in the same file           | `frame: false`                                                |
| Min size 620×500, default 910×600 | `AppWindow::Main` branch                      | `BrowserWindow` constructor options                           |
| Disable default file drop       | `.disable_drag_drop_handler()`                  | `will-navigate` guard in `window.ts`                          |
| Light theme on macOS            | `theme(Some(tauri::Theme::Light))`              | `nativeTheme.themeSource = "light"`                           |
| Draggable title strip           | `data-tauri-drag-region`                        | `-webkit-app-region: drag` strip in `src/renderer/router.tsx`       |
| Tray (Open / Quit)              | `plugins/tray/ext.rs::create_tray_menu`         | `src/main/tray.ts`, reusing `plugins/tray/icons/*.png`    |
| Single-instance focus           | `tauri_plugin_single_instance`                  | `app.requestSingleInstanceLock()` + `second-instance` handler |
| Dock reopen                     | `RunEvent::Reopen`                              | `app.on("activate", …)`                                       |
| Survive close on macOS          | `RunEvent::ExitRequested` → `ActivationPolicy`  | `window-all-closed` → `app.setActivationPolicy("accessory")`  |

Explicitly still out of scope (next slices, not de-risked here): deep-link
runtime handlers, updater, customized app menu, tray sub-items beyond
Open/Quit, shortcut/notification plugins, code signing & notarization.

## Build graph

`@hypr/desktop2` runs through Turbo so upstream packages rebuild before the app
itself. Running:

```sh
pnpm turbo run build -F @hypr/desktop2
```

transparently fans out to `@hypr/napi-sdk#build` upstream, then builds the
renderer and Electron entrypoints from `src/`.

## IPC contract

Shared DTOs between Electron main, preload, and the renderer live in
`src/shared/`.

- `src/shared/api.ts` defines the `HyprElectronApi` surface exposed on
  `window.hypr`
- `src/shared/channels.ts` keeps `src/main/main.ts` and
  `src/preload/preload.cts` on one source of truth for channel names
- `src/shared/updater.ts` holds the updater event types and reducer used by the
  renderer banner

## Subscription pipeline

Rust → renderer delta forwarding is handled by
`src/main/subscription-manager.ts`; `main.ts` only wires IPC endpoints.
Each `subscribe()` call mints a private channel so deltas don't cross-talk
between subscribers, and closed windows clean up their own NAPI handles.
See `src/main/subscription-architecture.md` for the full rationale.

## Packaging

Distribution is `electron-builder` (not Forge). Config is TS, at the app
root: `electron-builder.config.ts`. `pnpm bundle` passes it explicitly with
`--config`. The `package.json#build` key is intentionally absent so there's
one place to edit signing / notarize / channel / publish settings when we
wire those up.

## Local data path

`desktop2` intentionally does **not** share the existing desktop database.

- Electron uses `app.getPath("appData")`
- Then writes to `com.hyprnote.desktop2/app.db`

This keeps the Tauri app and the Electron app isolated while the transport and
schema work is still in flux.

## Scripts

Invoke through turbo so `@hypr/napi-sdk` (NAPI binding) is built-and-cached
upstream:

- `pnpm turbo run dev -F @hypr/desktop2`
- `pnpm turbo run build -F @hypr/desktop2`
- `pnpm turbo run bundle -F @hypr/desktop2`

`pnpm -F @hypr/desktop2 <script>` still works but will **not** rebuild
`@hypr/napi-sdk`, so you'll get stale NAPI bindings if the Rust crate changed.
Use `pnpm turbo run build -F @hypr/napi-sdk` manually in that case.

### Orchestration layout

- **turbo** owns the cross-package graph: `dev`/`build`/`bundle` declare
  `"dependsOn": ["^build"]` so `@hypr/napi-sdk` (and anything it transitively
  needs) builds first, with input/output caching.
- **concurrently** owns the runtime: inside a single `dev` invocation it runs
  `vite` and the Electron launcher in parallel with cooperative Ctrl-C.
- **wait-on** owns the port-readiness handoff from Vite to Electron.

## Current behavior

- CRUD calls from the Electron renderer go through preload IPC into Electron main,
  then into `@hypr/napi-sdk`.
- Reactive updates also come from `@hypr/napi-sdk`, via a N-API callback wired
  into the same `hypr_db_reactive` runtime as local writes.
- Electron main forwards those deltas to renderer windows through
  private `hypr:liveQuery:delta:<uuid>` channels owned by
  `LiveQuerySubscriptionManager`.

## Explicit non-goals

- No Tauri plugins are ported; native shell behaviors are re-implemented
  directly against Electron APIs (see the parity table above).
- No TinyBase integration is kept in this shell.
- No code signing, notarization, or auto-update flow is configured.
- No feature parity with `apps/desktop` beyond the `sessions` proof slice and
  the native shell primitives listed above.

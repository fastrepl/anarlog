# desktop-gpui

Native [GPUI](https://gpui.rs) shell for the Anarlog desktop app. This is the
landing zone for migrating `apps/desktop` off Tauri (ANLG-320). It runs side by
side with the Tauri app and reads the same SQLite database.

## Current scope

- Opens the Tauri app's `app.db` **read-only** (same path resolution as
  `apps/desktop/src-tauri/src/db.rs`; `--db-path` overrides it).
- Lists sessions from `anlg-db-app` and renders the selected session's note
  (ProseMirror JSON is converted to Markdown with `anlg-tiptap`).
- No writes, no migrations, no recording, no LLM/STT. The Tauri app stays the
  schema owner until the write path moves here.

## Run

```sh
cargo run -p desktop-gpui
cargo run -p desktop-gpui -- --db-path /path/to/app.db
```

Debug builds open the `com.hyprnote.dev` database, release builds
`com.hyprnote.stable`; pass `--identifier` to override.

### Linux build prerequisites

GPUI needs `libxkbcommon` headers at link time and a Vulkan driver at runtime
(`mesa-vulkan-drivers` provides lavapipe for headless/VM use). Everything else
(fontconfig, Wayland, Vulkan loader) is `dlopen`ed.

```sh
sudo apt-get install -y libxkbcommon-dev libxkbcommon-x11-dev mesa-vulkan-drivers
```

macOS needs Xcode with the Metal toolchain (`xcodebuild -downloadComponent MetalToolchain` on Xcode 26).

## Layout

```
src/
  main.rs       args, tokio runtime, GPUI Application + window
  db.rs         app.db path resolution, read-only Store, note body conversion
  workspace.rs  root view: sessions sidebar + note pane + status bar
  theme.rs      palette
```

`Store` runs sqlx futures on a dedicated tokio runtime and hands GPUI a
`JoinHandle` to await on its foreground executor; views never block the UI
thread on the database.

## Dependency policy

`gpui` is pinned to the crates.io release published by Zed Industries rather
than a git revision of `zed-industries/zed`, so `--locked` builds stay
reproducible and CI does not clone the Zed monorepo. Bumping the version is a
one-line change in the workspace `Cargo.toml`; expect API churn between
releases while GPUI is pre-1.0.

## Migration plan

Each step keeps the Tauri app shippable and moves one capability over:

1. Read-only shell (this crate): sessions list + note view.
2. Writes: session/document upserts through `anlg-db-app`, reactive updates via
   `anlg-db-reactive` instead of the webview live-query bridge.
3. Editor: ProseMirror-compatible rich text on top of GPUI text primitives,
   persisting TipTap-dialect JSON validated by `crates/tiptap`.
4. Recording + transcription: reuse `listener2-core`, `audio-*`, `local-stt-core`
   directly (they are already Tauri-agnostic).
5. Settings, calendar, templates, search (`tantivy`), tray/windows, updater,
   deep links: port plugin-by-plugin, replacing each `plugins/*` Tauri wrapper
   with a direct crate call.
6. Packaging: replace `tauri-build`/`tauri.conf.*.json` with a GPUI bundle
   pipeline; cut over once feature parity gates in `desktop_e2e` pass.

# desktop-gpui

Native [GPUI](https://gpui.rs) shell for the Anarlog desktop app. This is the
`apps/desktop-gpui` application called for in the target architecture of
[ANLG-320](https://linear.app/fastrepl-inc/issue/ANLG-320/migrate-the-desktop-application-from-tauri-to-gpui).
It runs side by side with the Tauri app, reads the same SQLite database, and is
not a replacement for anything yet.

Where this sits in the ANLG-320 plan: it is the start of **Phase 2 (GPUI
foundation)**, scaffolding the crate with test and CI targets, async runtime
integration, logging, and a minimal theme. It deliberately does not pre-empt
**Phase 0 (baseline, inventory, profiling, success thresholds)**, which gates
whether the migration proceeds at all; the shell only exists so the GPUI
dependency graph, build prerequisites, and data-layer reuse are proven before
Phase 3 needs them.

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

The crate lives in the root workspace because its dependency graph resolved
without conflicts. ANLG-320 allows isolating it in a nested workspace if pinned
Rust, wgpu, font, or platform dependencies ever collide with the root; any fork
or permanent patch needs a written rationale and an owner.

## Plan

The phases, gates, benchmark protocol, and stop conditions are owned by
ANLG-320; this README does not restate them. In short: Phase 0 baselines and
inventories the Tauri app, Phase 1 moves logic behind shell-neutral Rust
services that both Tauri and GPUI call, Phases 2 to 6 build the GPUI app up to
parity, and Phase 7 dual-runs both builds before any cutover. Tauri stays the
default and the rollback path throughout. Work in this crate must not change
the SQLite schema or the document format.

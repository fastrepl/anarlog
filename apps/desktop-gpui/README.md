# desktop-gpui

Native [GPUI](https://gpui.rs) shell for the Anarlog desktop app, the
`apps/desktop-gpui` application from
[ANLG-320](https://linear.app/fastrepl-inc/issue/ANLG-320/migrate-the-desktop-application-from-tauri-to-gpui).
It ships next to the Tauri app as an opt-in sidecar, opens the same SQLite
database, and ports the Tauri screens one surface at a time. Tauri stays the
default and the rollback path; nothing in this crate changes the SQLite schema
or the ProseMirror document format.

## Switching shells

The Tauri binary is the installed launcher. `Settings → General → Try the new
Anarlog (beta)` writes the shell preference (`anlg-storage::shell`) and
relaunches; on the next start the launcher execs `anarlog-gpui` from next to
its own binary (`apps/desktop/src-tauri/src/shell.rs`), forwarding its
arguments. `Settings → General → Switch back` in the native shell restores the
classic app. `ANARLOG_SHELL=tauri|gpui` overrides the marker for one run.
Linux builds ship the sidecar (`desktop_cd.yaml`); macOS and Windows follow once
their `desktop_ci.yaml` lanes gate this crate.

## What is ported

Each surface is verified side by side against the running Tauri app on the same
database, with Tauri's logic and test fixtures ported where they exist.

- Main window: title bar, sidebar timeline (calendar events, folders, search,
  filters, context menus, drag to folder), Folders / Calendar / Contacts /
  Templates / Automations / Stats tabs, floating chat CTA, toasts, dialogs.
- Note view: title, view switcher, enhanced notes and templates, the ProseMirror
  editor with writes (marks, lists, tasks, `@` mentions over the shared Tantivy
  index, autolink, paste), find and replace, meeting info, share panel, export.
- Transcript: word-level seeking, playback follow, speaker assignment, edit
  mode with merge, text selection menu, live partials, audio player.
- Recording: microphone / system capture through the shared listener runtime,
  live and batch transcription, floating recording bar, audio import, the
  recording-without-transcription warning, tray recording state.
- Settings: General (shell switch, autostart, tray), Account (signed-out),
  Stats, Teams, Appearance, Notifications, Transcription and Intelligence
  (provider catalogue, live model listing, key verification, connection
  health, reasoning effort), Dictionary, Meetings, Sync, Imports, Privacy,
  Permissions, Developers.
- Onboarding with background music and the welcome demo note (`Join & record`
  opens the demo with a loopback completion callback and stops the capture when
  the video ends).
- System tray (Linux GTK thread; `anlg-tray-core` shares icons, labels, and
  the agenda logic with the Tauri plugin), keyboard shortcuts, deep links
  (`anlg-deeplink-core` shares the URL routes and callback page), and
  single-instance forwarding: a second launch hands its `anarlog://` URLs to
  the running window over a Unix socket next to the database.

Not ported yet: the signed-in flows (auth callback, billing, CloudSync,
integrations, shared-note opens: the deep links are routed and logged), the
on-device STT/LLM model rows (Apple Silicon only, so their providers are hidden
elsewhere like Tauri does), rich clipboard embeds, and Windows single-instance
forwarding.

## Run

```sh
cargo run -p desktop-gpui
cargo run -p desktop-gpui -- --db-path /path/to/app.db
cargo run -p desktop-gpui -- anarlog-dev://focus   # forwarded to a running instance
```

Debug builds open the `com.hyprnote.dev` database, release builds
`com.hyprnote.stable`; pass `--identifier` to override. Positional arguments are
deep-link URLs.

Headless verification (CI, cloud VMs) needs a dummy PulseAudio sink for the
audio player and `crates/audio-mock` for capture; see the repository
`AGENTS.md`.

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
  main.rs         args, tokio runtime, single instance, tray + deep-link polling, window
  db.rs           Store: app.db path resolution, sqlx queries, note/transcript writes
  workspace.rs    root view state; one submodule per surface under workspace/
  editor/         ProseMirror-compatible document model and editor view
  recording.rs    listener runtime bridge (capture, live/batch transcription)
  deeplink.rs     URL routing, single-instance socket, loopback callback server
  tray.rs         system tray thread
  ai_*.rs         provider catalogue, model listing, credential checks, health
  theme.rs, ui.rs, squircle.rs, text_input.rs, text_area.rs   shared widgets
```

`Store` runs sqlx futures on a dedicated tokio runtime and hands GPUI a
`JoinHandle` to await on its foreground executor; views never block the UI
thread on the database. Shell-neutral logic that both apps need lives in
`crates/` (`tray-core`, `deeplink-core`, `search-index`, `listener-core`, …)
rather than being duplicated here.

## Checks

`desktop_ci.yaml` (`gpui_linux_ci`) runs:

```sh
cargo clippy --locked -p desktop-gpui --all-targets --no-deps -- -D warnings
cargo test --locked -p desktop-gpui
```

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

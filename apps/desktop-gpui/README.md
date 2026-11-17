# Native GPUI preview (ANLG-320)

This is an opt-in, standalone native package. The existing desktop app remains the
shipping application and the behavior reference. The implementation comes from PR
#7720 (`2fc8da4537`), ported to current `main` and to the pinned upstream GPUI
source.

## Run

The app is its own Cargo workspace (`desktop-gpui` and `runtime/`) with its own
lockfile and `rust-toolchain.toml`, because upstream GPUI needs a newer Rust than
the repository workspace. Run cargo from this directory:

```sh
cargo run --locked
# Or select a disposable profile / backup copy:
cargo run --locked -- --profile /absolute/path/to/sandbox
```

The default is `$HOME/.anarlog-gpui-sandbox/library.sqlite` (Windows:
`%USERPROFILE%\.anarlog-gpui-sandbox\library.sqlite`). There is no automatic
discovery or migration of the shipping application's live profile. `--profile`
is a directory, not a database filename. Only use an isolated directory or a
consistent SQLite backup; do not copy a live WAL database's main file alone.
The canonical schema is prepared with `db-app`. CloudSync starts disabled and
requires account sign-in, encryption setup and explicit enablement.

The integrated preview mounts the native workspace, TipTap JSON editor,
transcript/audio pane, and concrete cloud/local product services. The editor
autosaves through a background journal; save conflicts retain drafts. The note
header opens transcript/audio, share, and export surfaces; a note can also open in its
own standalone window with its own editor and save journal. Recording, recovery and
AI use the selected provider, secure credentials and running local models.
Missing configuration produces an error; credentials are never fabricated. Errors,
confirmations and app notices appear as shipping's stacked toasts
(`application/toasts.rs`); there is no status line.
Transcription and intelligence settings expose provider/model/base-URL choices
and a masked API-key input. Keys save separately to the system keyring; provider
choices use an atomic CAS transaction. Local models are selected by starting
them through the model manager.

Cloud sign-in requires `VITE_SUPABASE_URL` and `VITE_SUPABASE_ANON_KEY` at build
or runtime. `VITE_API_URL` and `VITE_WEB_APP_URL` can select another deployment.
The native sandbox uses separate keyring namespaces. Local engine executables
and bundled CLI/skills are resolved under `ANARLOG_NATIVE_RESOURCES` (or the
`resources` directory beside the executable); models are stored under the isolated
profile. Argmax also requires `ARGMAX_API_KEY`.

Pinned tabs are restored and saved with CAS through `app_settings.gpui_pinned_tabs`
inside the isolated profile. Malformed persisted pin data disables pin writes.
This does not import the shipping application's file-based pinned-tab store.
Title/editor/product drafts gate navigation and quit. Like shipping, closing the
main window (or the application-menu Quit) hides it and the app keeps running in
the tray; the first ⌘Q only shows the quit overlay. A second ⌘Q (or ⌘⇧Q) or a
confirmed tray "Quit Completely…" quits through the drain: it flushes all note
windows, stops/finalizes capture and awaits runtime drain. Failed saves retain
drafts and keep windows open. OS-forced termination remains outside this
guarantee; GPUI's generic shutdown callback has a 100 ms timeout.

GPUI is pinned to an upstream `zed-industries/zed` revision without a fork or patch;
crates.io `gpui` 0.2.2 predates upstream's accessibility integration. On Linux, GPUI needs
X11 or Wayland, a working Vulkan device/driver, xkbcommon, fontconfig, and FreeType.
Build-only verification does not establish that a particular GPU/desktop works.
macOS and Windows require their native toolchains and platform validation.
Use Ubuntu 24.04 or a matching recent PipeWire SDK: Ubuntu 22.04's 0.3.48 SPA
headers cannot compile the repository's `libspa 0.9.2` dependency. The native
Linux CI uses Ubuntu 24.04 and never launches a display or audio device.
The tray also needs GTK 3 and the AppIndicator runtime/development packages.

The app's lockfile started from the repository lockfile so shared crates resolve to
the same versions; only GPUI's own requirements (for example the ICU 2.3 line)
move ahead. The repository workspace and its lockfile are unchanged.

## Architecture

- `main.rs` owns CLI/profile selection and the native window.
- `application.rs`, `application_services.rs`, `native_events.rs` and
  `note_window.rs` own service construction, pane/window routing, shared
  capture/playback, recovery, persisted pins, menus/tray/shortcuts/deep links,
  update barriers and the close/drain flow. `application/lifecycle.rs` holds the one
  update state that the tray, the app menu and the sidebar update toast all render.
- `ui/` provides source-derived light/dark HSL tokens, the system font policy,
  embedded existing logo and Hugeicons, focus restoration, and a bounded native
  text input with UTF-16/IME entry points and clipboard operations. Single-line
  fields are bounded at 4096 bytes; multiline catalog fields at 128 KiB.
- `workspace/` has separate shell, virtual list and note entities. A list refresh
  does not reparse or replace the selected document. List and note reads use
  cancellation plus monotonically increasing request generations; stale results
  cannot replace the current view. A previous page remains visible while loading.
- `desktop-runtime` owns a two-thread Tokio executor on a separate coordinator
  thread, a 64-item FIFO admission queue and canonical DB services. SQL, disk I/O,
  parsing for document saves and transaction execution happen there, never in
  rendering or text replacement. Accepted writes survive dropped receivers.
  Queue saturation is an explicit `Busy` result; it never silently drops a write.
- Data reads reuse `db-app`. Atomic writes/CAS use `db-execute`. Reactive queries
  use `db-reactive` rather than recreating its invalidation rules.
  Dependencies are table/view/FTS-level, not row-level. The foundation watches
  `sessions` only, then refreshes the bounded current page. No global settings,
  templates, contacts or document reload is attached to this signal.
- `QueryWatch` coalesces replaceable snapshots in one watch slot. Errors use an
  eight-item reliable queue; overflow records a terminal error and terminates
  that subscription. At most 32 watches can be live per runtime. Callers must
  inspect both channels and `terminal_error()`.
  `unsubscribe().await` is the canonical hard delivery barrier.
- `shutdown().await` rejects new admission, drains all accepted work, runs
  registered flush participants and closes the pool before acknowledging.
  Flush participants run on the runtime and must not enqueue back into the
  closed queue. Normal window close awaits this acknowledgement. OS-forced
  termination and GPUI's time-limited application-quit path cannot guarantee it.

`DocumentSnapshot.body` preserves exact stored bytes and unknown fields.
The isolated native runtime uses one pooled SQLite connection. SQLite's commit
hook can notify a reactive reader before a large WAL write becomes visible to
another connection; serializing acquisition makes refreshes wait for the writer.
Watches also suppress identical snapshots. Increasing the native pool size needs
a verified post-commit delivery barrier first.

Library navigation caches at most eight aligned pages (456 rows each), bounded
by an 8 MiB payload estimate. A separate read-only SQLite connection checks
`PRAGMA data_version` before serving a page and invalidates on committed writes,
including writes from another process. The canonical query materializes a
bounded ID page before indexed row lookups. It still scans/sorts without a
canonical composite expression index; no schema change is made. Run the ignored
release fixture in `runtime/tests/performance.rs` for raw cached,
uncached and reference samples; these are not whole-application measurements.

## Remaining parity and validation gates

Screens are compared side by side with the shipping frontend on macOS (see Visual
parity below). Focus/drag/drop/IME parity, native accessibility, i18n, Linux and
Windows rendering, hardware/provider/keyring behavior, real floating windows and
signed packaging/update installation remain unverified. Upstream GPUI
now provides accessibility hooks, but the views do not publish roles, labels or text
yet. A Wayland shortcut portal is missing, and only macOS can hide the main window on
close; elsewhere closing it quits through the drain. Cloud attachment transfer/hydration, image
resize handles and full multiline soft wrapping remain incomplete. Shared-note
previews retain source JSON but do not hydrate published attachments. This is
not release-ready and does not switch the shipping default.
Native localization, spellcheck and keyboard/drag-selection parity still need
work.

`save_document` is an explicit JSON-document API, not a lossless editor
implementation: it verifies the root and CASes the original body, format and
timestamp, but cannot prove that a caller retained every unknown node. The editor
lane must preserve unknown data and refuse unsafe transformations. CAS failures
must retain the dirty draft and expose reload/merge recovery.

## Visual direction

The native app is intentionally sharper than shipping, like the web app: every corner is
square (buttons, inputs, menus, dialogs, chips, toggles, avatars, dots and painted shapes),
and motion is near-instant. Functional transitions, such as a toast entering or leaving,
the chat panel's reveal and the quit overlay's fade, take 70 ms with linear easing;
decorative reveals are gone. Continuous indicators (spinners, audio meters, the recording
pulse and progress) keep their timing. `clippy.toml` disallows GPUI's rounding helpers and
painted corner radii. The shipping desktop app keeps its rounded look, so parity
comparisons judge layout, copy and behavior, not radii or easing.

## Visual parity

Both apps render the same seeded library without opening a window:

```sh
# Native screens (and a copy of the seeded library) into /tmp/screens
ANLG_SCREENS_DIR=/tmp/screens cargo test --test screens
# Shipping frontend with Tauri IPC mocked over a disposable copy of that library
cp /tmp/screens/library.sqlite /tmp/shipping.sqlite
PARITY_DB=/tmp/shipping.sqlite node parity/serve.mjs &
node parity/shoot.mjs http://127.0.0.1:1499/app/main /tmp/screens/shipping-main.png
node parity/shoot.mjs http://127.0.0.1:1499/app/main /tmp/screens/shipping-note.png \
  --click "Weekly product sync"
```

`tests/screens/*.sql` hold the fictional library, applied in name order. Tests that
drive `anlg-audio-mock` stay silent through `.cargo/config.toml` (`MOCK_PLAYBACK=0`). The shipping capture uses
headless Chromium from the Playwright cache (`CHROME` overrides it) and prints
console errors plus any IPC command it had no answer for.

## Module contracts

All modules are registered in `lib.rs` and mounted by `application.rs`.
The following contracts and ownership boundaries were used by the five fresh
implementation lanes. Their shared wiring now lives in the host modules above.

### Path ownership

| Owner | Exclusive paths |
| --- | --- |
| Workspace | `apps/desktop-gpui/src/workspace/**` |
| Editor | `apps/desktop-gpui/src/editor/**` |
| Meeting | `apps/desktop-gpui/src/meeting/**` |
| Product | `apps/desktop-gpui/src/product/**` |
| Runtime | `apps/desktop-gpui/runtime/**`, `apps/desktop-gpui/src/runtime_bridge.rs`, `apps/desktop-gpui/src/platform/**` |
| Final integrator | Root/shared manifests and lockfiles, all CI, `apps/desktop-gpui/{Cargo.toml,build.rs,README.md,scripts/**,assets/**}`, `src/{main.rs,lib.rs,contracts.rs,ui/**}`, `runtime/Cargo.toml` |

Workspace can create `workspace/theme.rs` for application-specific layout or
theme state, but must consume common `ui::theme::Theme`, not duplicate token
values. Existing domain crates and existing desktop/web/mobile code are outside
every lane's write ownership. Needed changes/extractions there, including
transport-neutral Tantivy, go to the integrator with the exact proposed patch.
No lane changes schema, credentials, release defaults or security policies.

### Shared symbols

Imports below are public from the `desktop_gpui` and `desktop_runtime` libraries:

```rust,ignore
use desktop_gpui::contracts::{
    LaneContext, EditorInit, EditorEvent, MeetingIntent, MeetingEvent,
    ProductRoute, ProductEvent, WorkspaceEvent,
};
use desktop_runtime::{
    RuntimeHandle, Profile, Reply, Services, CancellationToken, Generation,
    SessionId, DocumentId, HumanId, AttachmentId,
    SessionSummary, LibraryQuery, LibraryPage, OpenSession, DocumentSnapshot,
    RenameSession, SaveDocument, LoadState, ServiceError, QueryWatch,
};
```

`LaneContext { runtime: RuntimeHandle }` is cheap to clone. Immutable snapshots
use `Arc`; resource IDs are typed strings so imported IDs are not forced into a
UUID format. `LoadState<T>` distinguishes pending/ready/error/unsupported and
retains previous snapshots. `Reply<T>::receive().await -> Result<T>` can be
awaited by a GPUI task; it does not require a Tokio runtime on the UI thread.

`RuntimeHandle` APIs:

```rust,ignore
start(Profile) -> std::io::Result<(RuntimeHandle, Reply<()>)>
library(LibraryQuery, CancellationToken) -> Result<Reply<LibraryPage>>
open_session(SessionId, CancellationToken) -> Result<Reply<OpenSession>>
create_note(Arc<str>) -> Result<Reply<OpenSession>>
rename_session(RenameSession) -> Result<Reply<OpenSession>>
save_document(SaveDocument) -> Result<Reply<DocumentSnapshot>>
watch_library() -> Result<Reply<QueryWatch>>
watch_query(String, Vec<serde_json::Value>) -> Result<Reply<QueryWatch>>
submit(FnOnce(Services) -> SendFuture<Result<T>>) -> Result<Reply<T>>
read(CancellationToken, FnOnce(Services) -> SendFuture<Result<T>>) -> Result<Reply<T>>
register_flush(FlushParticipant) -> Result<Reply<()>>
shutdown().await -> Result<()>
```

`submit`/`read` are worker entry points, not UI SQL APIs. Domain-specific
typed operations belong in runtime submodules. Independent long-running service
work uses a 16-item queue with at most four concurrent jobs, separate from the
serialized database coordinator.
`register_flush` must be acknowledged before starting a durable producer.
Cancellation is for superseded reads, never an accepted authoritative write.

### Constructors and event routing

Each constructor below returns `Self`, called through `cx.new(|cx| ...)`.
Views implement GPUI `Render` and the indicated `EventEmitter<Event>`.
Subscribe with `cx.subscribe` and retain the `Subscription`.
Workspace also implements `Focusable`; use its focus handle for
`EditorInit::return_focus`.

| Lane | Public constructor |
| --- | --- |
| Workspace | `workspace::WorkspaceView::new(LaneContext, Reply<()>, &mut Window, &mut Context<Self>)` |
| Editor | `editor::EditorPane::new(LaneContext, EditorInit, &mut Window, &mut Context<Self>)` |
| Meeting | `meeting::MeetingPane::new(LaneContext, MeetingIntent, &mut Window, &mut Context<Self>)` |
| Product | `product::ProductPane::new(LaneContext, ProductRoute, &mut Window, &mut Context<Self>)` |
| Runtime | `runtime_bridge::start(Profile) -> std::io::Result<(RuntimeHandle, Reply<()>)>` |

The integrator starts the runtime once, gives the readiness reply only to
workspace, and clones `LaneContext` into every other pane.

- Workspace emits `WorkspaceEvent::OpenEditor { session_id, document }`,
  `Meeting(MeetingIntent)` and `Product(ProductRoute)`. The final integrator
  creates/focuses the corresponding pane. Workspace owns session/folder
  selection, tabs, navigation history, search, contacts, calendar, templates and
  automations. It does not own document JSON or recorder internals.
  The host handles editor, meeting, product, shared-preview and standalone-window
  routes and forwards saved-note/capture events to automation execution.
- Editor receives `EditorInit { session_id, document, return_focus }`. It emits
  `Dirty { session_id, dirty }`, `Saved(DocumentSnapshot)`,
  `SaveFailed { session_id, error }`, `OpenLink(Arc<str>)`,
  `OpenAttachment(AttachmentId)` and `MentionHuman(HumanId)`.
  Dirty/save failure is authoritative and must not be coalesced away.
  The integrator handles external navigation and close guards. Native selection,
  rich text, UTF-16 mapping, IME/CJK, history, menus, clipboard, attachment
  insertion, lossless unknown nodes, concurrent sync and autosave belong here.
  The shared single-line input is not the rich-text editor.
- Meeting receives `MeetingIntent::{Open(SessionId), Start { session_id }, Stop}`.
  It emits `Recording { session_id, active }`, `OpenSession(SessionId)` and
  `Failed(ServiceError)`. The intent enum is a request, never proof that capture
  started. Runtime/audio workers must acknowledge start/stop and durable final
  transcript events. Meeting owns devices/capture, live partial/final words,
  speakers, retention including Never, recovery, playback/waveform, summaries and
  local/hosted AI execution. Keep transcript delivery reliable and partial UI
  snapshots coalesced independently.
- Product receives `ProductRoute::{Onboarding, Permissions, Settings, Account,
  Billing, CloudSync, Share(SessionId), Import, Export(SessionId), Models,
  Integrations}`. It emits `NavigateWorkspace`, `OpenSession(SessionId)` and
  `Failed(ServiceError)`. Auth/E2EE/settings orchestration, imports/exports and
  product integration state belong here. Never represent an unavailable
  provider, permission, license or secret as successful.
- Runtime owns paths/profile policy, native platform adapters, reactive
  subscription lifecycle, cancellation, startup/drain barriers, localization and
  accessibility foundations, and release-build instrumentation/fixtures.
  `platform::WindowRole::{Main, Settings, MeetingOverlay}` and
  `PlatformEvent::{DeepLink(String), Shortcut(String), OpenMainWindow,
  Failed(ServiceError)}` define the native boundary. Concrete adapters are
  installed by the host; unsupported platform cases remain explicit errors.

### Assets and theme

`build.rs` derives all HSL colors from
`packages/design-system/src/tokens.css`; the radius token is unused (see Visual
direction). The CSS RGBA keyboard-shadow tokens are not yet native shadow primitives.
`ui::theme::UiFonts` uses San Francisco and SF Mono on macOS (shipping's `system-ui`
and `ui-monospace`), and `Segoe UI` or Linux `sans-serif` elsewhere. Use
`ui::theme::MEDIUM` for `font-medium`: font-kit reads SF's Medium face as weight 530.
The existing logo is embedded directly. Hugeicons SVGs are generated from the
repository-pinned `@hugeicons/core-free-icons` package:

```sh
node apps/desktop-gpui/scripts/export-icons.mjs
```

Keep generated SVGs checked in so Rust builds do not require Node. Ask the
integrator to extend the generator/asset map for additional icons. Do not add
an unrelated icon family or replace the repository font policy.

## Verification and remaining gates

From `apps/desktop-gpui` (matches `.github/workflows/gpui_ci.yaml`):

```sh
cargo fmt --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets --no-deps -- -D warnings
cargo build --locked
```

The runtime tests use temporary databases only, covering durable reopen,
opaque JSON preservation, CAS conflict and deleted-document rejection,
queue saturation/drain, flush barriers, cancellation and reactive unsubscribe.
Input model tests cover UTF-16, grapheme deletion and composition-relative ranges.
These do not validate native input methods or accessibility. Integration coverage
also exercises rich-document save/reopen through `SaveJournal`, preservation of
opaque/table/task content, competing writes, explicit conflict resolution, and
pin CAS/corrupt-storage behavior.

`pnpm dev:gpui --profile /absolute/path/to/sandbox` runs the native binary.

Equivalent release-build old/new profiling with reproducible large-library,
large-document and streaming fixtures is required before claiming any speed,
memory or responsiveness improvement. A successful Linux build is only a
build-viability result. See the remaining parity and validation gates above.

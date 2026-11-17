# Char meeting internals migration

Planning date: November 17, 2026. Implementation authorized after this plan. Tracking: [ANLG-560](https://linear.app/fastrepl-inc/issue/ANLG-560/port-chars-meeting-capture-speaker-labels-notices-and-camera-into).

Replace Anarlog's macOS meeting capture and the affected meeting-app integrations with Char's implementations. Preserve Anarlog's UI, sessions, transcript providers, manual speaker assignments, consent records, recovery, retention, and sync behavior. Add the camera watermark as a native capability. This is a source port, not an Electron application or UI port.

## Decisions and boundaries

- The user's current request authorizes platform-derived speaker labels and supersedes the August 23 voiceprint-only restriction recorded in canceled ANLG-81 and ANLG-294. Keep those issues closed. Platform observations are session evidence; they neither enroll a voiceprint nor create or merge People automatically. Manual assignments and consented durable identity remain authoritative.
- Keep shared behavior in Rust and the capture/camera engines in Swift. Tauri commands are adapters. The applied GPUI work already consumes AudioProvider; keep shared contracts usable there without editing other agents' GPUI views or application wiring.
- macOS is the first implementation target because Char's native capture, AX automation, and camera extension are macOS code. Both current capture packages target macOS 15. Keep existing Windows/Linux backends and compile paths. This is not a claim of feature parity on those platforms.
- Use one audio backend per capture. Keep an internal opt-in during qualification; do not change the production default until the acceptance gates pass. Never silently switch engines or broaden an app-scoped recording to whole-system audio after a failure.
- Preserve selected microphone behavior. Char currently requires the selected device to be the system default. Either implement and test equivalent selection in the port or retain the existing backend for that explicit selection with an honest capability result. Do not change the system default microphone behind the user's back.
- No push, PR, merge, release, deployment, extension installation, real microphone/camera capture, or live chat send is part of this implementation batch. Local builds, synthetic fixtures, and local commits are authorized. Live qualification remains a separately reported step.

## Frozen source

Char source commit: `d4608723d8ac328ab91091803a08c4585a0dc244`.

The selected committed source is exported to `/tmp/anarlog-char-migration-20261117/char-source`; its manifest is `/tmp/anarlog-char-migration-20261117/source-manifest.json`. This is a reference export without Git metadata, not a child checkout. Uncommitted Char work is excluded. The source commit contains these reviewed branch heads:

- Audio: `b398e4bfa44bd287843485d93ce16fa4bf29a68d`.
- Speakers: `2a018784f5d2e2f6ef5ca03c70a8e6ed9e679509`.
- Meet attachment: `12bf21bcbd3ceb763a9f3495b803135574037ec0`.
- Notices: `729e5ed21f6cad56292b7c5bcee64d571dfc777d`.
- Watermark: `95b5d6723948bb7f53e042413b61d1a4f5a4e567`.

Each lane records the source paths it actually ports and preserves required license notices. The combined source contains subsequent work and must be validated during the port; earlier Char test counts are not Anarlog acceptance evidence. Do not import unrelated Char code, provider infrastructure, People/Page models, or UI assets.

## Shared contracts

The integration owner publishes the concrete Rust signatures before modifying consumers. Prefer small additions to existing crates over a new framework.

1. Capture identity is account/library + session ID + a fresh capture generation. Session IDs alone are insufficient because a session can be recorded repeatedly. Every asynchronous completion checks the active generation and account before acting.
2. A confirmed call contains process ID, bundle/app identity, platform, and optional canonical call URL, obtained from the same native observation used to choose capture scope. Unknown, ambiguous, stale, or mismatched context cannot authorize attachment or automation.
3. Audio and speaker observations share a capture-relative monotonic timeline, mapped explicitly to existing transcript offsets. Preserve sequence, gaps, start offsets, pause intervals, and provider reconnect generations. Never join evidence across a pause or reuse a previous capture's call.
4. Audio exposes separate microphone and remote tracks plus channel readiness/failure and processing provenance. Char's 48 kHz samples are converted once at Anarlog's 16 kHz live-consumer boundary. Apple-processed audio must not be presented as raw input or run through AEC again.
5. A speaker observation identifies its source, capture, timestamp, visible participants/self flags, optional total count, and explicit speaking cues. A name in a roster is not a speaking cue. Unknown signals remain absent.
6. Recording lifecycle ownership is app or extension. A toolbar click may attach to an app-owned capture only on the confirmed call. Link tokens scope later status/control/cue/outcome requests; stale tabs are refused. Detaching or leaving an attached call preserves app-owned recording; explicit Stop still stops it. Extension-owned captures retain their own stop-on-detach behavior.
7. Notice results distinguish sent, definitely not sent, cancelled before dispatch, and unconfirmed after dispatch. Once a send may have happened, retries are forbidden. Persist the account/meeting claim before dispatch and retain it through uncertain outcomes and restarts. Operation cancellation must reach native code.
8. Camera state distinguishes requested configuration from emitted output. An app effect can be disabled only after a fresh output receipt for the exact current background request, and after rechecking intent. A stale helper, missing receipt, cancellation, or unreadable app effect preserves privacy and yields accurate guidance.

## Implementation lanes and ownership

All six lanes use the shared Anarlog GitButler workspace. Each uses its named local branch, commits only its files/hunks, and leaves other work untouched. No branch switching, worktree creation, pulls, or history rewriting. A dependency requiring stacking is reported to the coordinator; do not solve it by sweeping other changes into a commit.

### 1. Shared contracts and integration

Branch: `refactor/char-meeting-integration`.

Owns root Cargo/workspace/lockfile edits, shared dependency registration, `crates/audio/src/lib.rs`, meeting-context coordination in `crates/transcription-core`, `plugins/transcription`, Tauri startup/capabilities/bundle configuration, shared settings registration, and packaging/workflow integration. Owns generated transcription bindings. Other lanes provide leaf manifests and exact registration requests.

Publish the capture/event signatures first. Wire one runtime owner for audio, context, speaker observations, notice dispatch, and camera state; do not run separate lifecycle owners in React and Rust. Adapt current UI-facing commands without replicating Char's UI. Preserve local journal/retention semantics and crash recovery, and make new helper files part of the same cleanup ownership. Avoid orphaned second recording stores. Compile shared consumers, including the native desktop where locally feasible, without taking over its unrelated work.

Build the signed-helper resource layout and capability reporting for direct distribution and App Store separately. Missing signing profiles are a reported packaging limitation, not a reason to fake readiness or request secrets. The first integrated milestone is an Anarlog synthetic recording through the new engine that renders through existing transcript contracts and survives recovery.

### 2. Native audio

Branch: `refactor/char-meeting-audio`.

Owns new `tools/meeting-capture`, a narrow Rust native-helper adapter (prefer `crates/audio-actual/src/native_meeting`), and source-loop changes in `crates/listener-core` needed by the adapter. Shared `crates/audio/src/lib.rs` edits belong to integration. Do not alter dictation's backend by accident.

Port CaptureCore, AudioEngineGuard, MeetingAccessibility, and the minimal PCM/control CLI from the frozen source. Avoid importing Char's optional speech/diarization stack when Anarlog's selected providers already supply it. Preserve device-clock placement, converter priming, bounded queues, track separation, interruption recovery, and the synchronized journal. Translate helper output into the agreed AudioProvider events. Keep device errors, no buffers, and actual silence distinct. Retain raw staging files until Anarlog acknowledges durable adoption, then honor its retention and deletion policy.

Verify timing with mismatched-rate/drift fixtures, route interruption events, pause clipping, short/empty recordings, helper termination, disk/write failure, and selected-device compatibility. Speaker ducking is unresolved in the source: leave the automatic default unchanged inside the opt-in port and expose trial modes only for qualification. No hardware trials in this batch.

### 3. Meet extension and bridge

Branch: `refactor/char-meeting-browser`.

Owns `browser-meeting-extension`, `crates/chrome-native-host`, and a shell-independent browser protocol module in `crates/transcription-core/src/browser_meeting`. Integration owns its module declaration and application registration. Replace the current state-file-only host; do not reuse Char's app paths, host identity, loopback assumptions, or account secrets.

Port only meeting scripts and the required manifest/background/native-messaging flow. Preserve toolbar activeTab permission, exact canonical Meet matching, browser-family verification, link tokens, stale-link takeover, detach ownership, cue-reader cleanup, notice/camera outcomes, and operation handout deduplication. Use a narrowly scoped authenticated local transport; no unauthenticated general recording-control endpoint. The extension handles Meet actions when explicitly attached; native dispatch must not also post or switch the same capture. Route actions through the shared runtime, not a second recorder. Browser audio remains process-scoped, not per-tab isolated.

Test wrong call/profile/token/generation, navigation and removal during start, host restart, attach/detach/pause/stop, and late outcome delivery with fixtures. Provide explicit native-host registration/bundling requests to integration.

### 4. Speaker attribution

Branch: `refactor/char-meeting-speakers`.

Owns `crates/transcript` attribution and evidence types and `apps/desktop/src/stt/speaker-context*` adapters/tests. Persistence and generated plugin contract changes are coordinated through integration.

Port whole-voice voting, provisional thresholds, stale/conflicting cue guards, strict 1:1 roster fallback, and final reconciliation into Anarlog's existing transcript/identity representation. Preserve manual assignments and consented voiceprint identity. Scope provider voices across reconnects and captures. Apply label changes using Anarlog's stable word/hint contracts; do not transplant Char's `:named` replacement IDs if that would duplicate Anarlog words. Re-run final reconciliation even when live transcription is unavailable, while retaining the existing batch-repair request.

Meet supplies measured active-speaker evidence. Native Zoom/Teams/Webex speaking signals remain unavailable until live evidence proves them. Only currently proven complete rosters may trigger 1:1 labeling; today's fixtures qualify Webex with its participant panel open. Reuse replay/projection tests, including manual correction, two voices for one participant, missing count/self, stale cues, pause, and provider rewrites.

### 5. Native notice delivery and claims

Branch: `refactor/char-meeting-notices`.

Owns `crates/detect` native meeting-chat automation, its shared `meeting_automation` helpers/lock, `plugins/detect`, `apps/desktop/src/stt/meeting-disclosure.ts`, related consent delivery adapters/tests, and narrowly scoped new Rust DB operations/additive migration for local notice claims. Owns detect bindings/permissions generation. Camera gets a separate native module and must use the same automation lock; coordinate module declarations with its owner.

Port exact-call targeting, draft/focus protection, one send action, native cancellation, and uncertain-result handling. Persist a local account-scoped claim before dispatch; release only definitely-unsent outcomes, fail closed on storage errors, and recheck account/capture after asynchronous writes. Keep Char's seven-day retention explicit. Existing disclosure attempt CHECK constraints cannot represent uncertainty: add compatible local-only storage rather than mislabeling it or rewriting a shipped migration. Keep consent state and stop-on-decline semantics separate; no CloudSync replication of transport claims.

Tests cover restart/crash windows, cancellation before/after send, simultaneous starts, changed drafts, account changes, malformed storage, and repeated recordings. Reuse a fake native chat input; never send to a real meeting. Existing chat capture, screen capture, and unrelated platform support remain intact.

### 6. Camera watermark

Branch: `refactor/char-meeting-camera`.

Owns new `tools/camera-watermark`, its helper protocol and minimal Rust/Tauri adapter in a new `plugins/camera-watermark` following add-plugin, and a dedicated `crates/detect/src/meeting_camera` module. Integration owns root registrations, Tauri capabilities/bundle entitlements, and existing settings-page integration. Native module exports/shared lock are coordinated with the notice owner. Do not edit other agents' settings views.

Port the CoreMediaIO extension, helper, compositor/background processing, output-receipt protocol, and guarded switching. Use Anarlog names, bundle IDs, app groups, state paths, and branding; it must coexist with Char Camera. Supply a minimal enable/background/switch API for existing settings, not Char's settings UI or preset gallery. Keep explicit opt-in, leave an off camera off, preserve manual choices, and report unsupported or unavailable automation truthfully. Meet/Webex effect removal remains manual until real fixtures support it.

Test protocol freshness, request mismatch, stale helpers, cancellation, concurrent user background changes, and switch failure. Build helper/extension without installing or activating them. Provide signing/profile/entitlement requirements as concrete packaging inputs; no claim of visible remote video without live acceptance.

## Execution order and integration handoffs

Start all six bounded lanes after this plan is saved. Independent engines, scripts, algorithms, and tests can proceed while integration publishes shared signatures. A lane with an unresolved shared API implements its independent part and sends an exact requested signature/path to the coordinator; it does not invent a competing runtime or edit another lane's files. Record signatures in `/tmp/anarlog-char-migration-20261117/contracts.md` and lane handoffs in that directory. Reports use `<lane>/REPORT.md` with source paths, local commit, tests, failures, and integration requests.

Each lane runs the relevant checks in AGENTS.md and current workflows with the testing skill. Keep formatting scoped. Cargo checks use `--locked`; only integration edits the root lockfile. Generated contracts come from their generator. The coordinator reviews actual diffs and regression evidence, then schedules the final wiring/verification pass once the necessary lane commits exist. An independent review is delegated only after a concrete combined result exists.

## Acceptance and cutover

- Automated: drift/resampling, monotonic timestamps, pause and gaps, capture generation, reconstruction after helper failure, retained-audio cleanup, source isolation, conservative speaker labels, exact-call/token attachment, durable notice claims, and background-request receipts. Regressions must fail against the prior behavior where applicable.
- Compatibility: existing sessions/transcripts remain readable and editable; manual speaker corrections survive live and batch processing; no changed retention/sync ownership; old platform backends and shared consumers still compile. New local schema is additive and explicitly local-only.
- Packaging: Anarlog helper identities, permissions and registration are verified for each distribution target. No helper executable or Char extension identity is accidentally reused from another installed app. CI's native jobs must be checked explicitly when publication is later authorized; green PR checks alone do not cover them.
- Live qualification, still pending: signed-app TCC, selected/wired/USB/Bluetooth/AirPods input/output changes, long recordings, speaker echo and audible ducking, real Meet attach/other-call refusal, native focus/draft/cancellation, installed camera receipt propagation, and outgoing video throughout the effect transition.
- Default cutover occurs only after the relevant live and compatibility gates. Remove superseded macOS paths only when no supported mode still requires them. Keep unsupported native speaker signals and manual Meet/Webex effects visible as limits. This implementation batch can finish with an integrated opt-in path and explicitly pending live qualification; it cannot claim production readiness from fixtures.

---
name: qa-critical-ux
description: QA-test the critical desktop user experience before a release — auth, CloudSync, calendar connect + notifications, note creation, recording, chat, and automated summaries across on-device, API-key, and Pro providers. Use before cutting a stable release, after changes to auth/CloudSync/STT/enhance/calendar/billing flows, or when asked to "QA the app".
---

# QA: Critical User Experience

Gate releases on this checklist. Every item must pass (or be explicitly
waived by the user) before running the release-new-version skill.

## Setup

1. Build and launch an authenticated native Dev bundle with AEC diagnostics:

   ```bash
   .agents/skills/qa-critical-ux/scripts/run-native-dev-qa.sh
   ```

   The script reads the currently deployed public Supabase configuration,
   derives the production app/API endpoints from `desktop_cd.yaml`, builds the
   native app identity needed for Computer Use, and launches with
   `AUDIO_SYNC_PROBE=1` and `LISTENER_DEBUG=1`. The build runs with an
   allowlisted environment containing only the public frontend configuration
   and minimal process/toolchain values. It does not load `.env.supabase`,
   desktop `.env` files through `dotenvx`, server credentials, or unrelated
   channel configuration. Local environment files are also excluded from the
   provenance fingerprint.

   Native Dev release evidence is invalid unless `VITE_APP_URL`,
   `VITE_API_URL`, `VITE_SUPABASE_URL`, and `VITE_SUPABASE_ANON_KEY` all match
   the currently deployed production public values. Local development URLs or
   a different Supabase project require a rebuild and cannot pass this gate.

   The helper owns a clean, persistent Cargo cache under
   `~/Library/Caches/anarlog`; never point it at the repository's `target`
   directory or clone that directory into the QA cache. The repository cache
   can contain enough old, provenance-tracked proc-macro dylibs for macOS
   assessment to stall `rustc` for minutes. Native builds use the full Xcode
   toolchain explicitly so Swift/MLX can invoke the Metal compiler instead of
   inheriting Command Line Tools. The helper also builds the workspace UI
   package before Tauri, so a clean checkout does not depend on generated
   `packages/ui/dist` output from an earlier build.

   A successful-build manifest fingerprints the complete app bundle, all
   desktop build inputs (including legacy crates), and the deployed public
   auth key. Before the manifest is written, the helper inspects the generated
   frontend `runtimeEnv`, rejects any non-allowlisted public variables, and
   binds that validated configuration fingerprint to the app bundle hash. This
   prevents `--launch-only` from running a stale bundle, a locally configured
   bundle, or a bundle whose production auth config rotated. Reuse an
   already-current bundle with:

   ```bash
   .agents/skills/qa-critical-ux/scripts/run-native-dev-qa.sh --launch-only
   ```

   Pin release-gate builds to the intended 40-character candidate commit:

   ```bash
   ANARLOG_QA_GIT_SHA=<candidate-commit-sha> \
     .agents/skills/qa-critical-ux/scripts/run-native-dev-qa.sh
   ```

   In a GitButler workspace, copy the selected branch tip's full `commitId`
   from `but status --format json`; `git rev-parse HEAD` is a synthetic
   workspace commit and is not release provenance. Without the variable, the
   helper auto-derives only when exactly one GitButler stack is applied, using
   that stack's top branch tip. Multiple applied stacks or a stack without a
   committed tip fail closed. Use the same variable with `--launch-only`.

   The first run uses a cold native cache and can take longer. Later source
   builds reuse only that helper-owned cache; launch-only performs validation
   without rebuilding.

   A newly rebuilt ad-hoc Dev bundle can prompt for access to the E2EE recovery
   key because its code-signing hash changed. Approve **Always Allow** once;
   `--launch-only` keeps the same binary and avoids another prompt. Staging and
   stable use a persistent Developer ID identity and do not have this Dev-only
   behavior.

   The helper connects the Dev app identity and its existing local database to
   production services. Use the intended QA account/workspace, and never commit
   channel credentials.
2. Do not repurpose this helper as a staging build. Staging release evidence
   must come from the signed, notarized artifact produced by
   `desktop_cd.yaml`; use the exact-run handoff below.
3. Sign in with a test account that has calendar access. For provider
   matrix runs you need: a Pro (or trialing) account, an API key for at
   least one cloud provider (e.g. OpenAI), and a downloaded local STT +
   LLM model pair.
4. Note the app version and the provider config under test in the report.
5. For macOS audio regression runs, leave the MacBook open and use its
   built-in speakers and microphone with no external audio device attached.
   The Dev helper fails before launch unless both macOS default devices use
   the built-in transport; do not bypass that preflight.
6. Play the 15-minute Lex Fridman fixture from a long-lived terminal
   command after recording starts:

   ```bash
   /usr/bin/afplay -v 0.7 \
     "$PWD/crates/data/src/english_10/audio.mp3"
   ```

   Let it finish naturally, or stop it with Ctrl-C. Do not use QuickTime or
   Computer Use just to control fixture playback.

## Release-candidate order

1. Run the complete Dev checklist from a clean checkout of the exact committed
   SHA. Do not clean, discard, or include unrelated GitButler workspace changes
   to produce release evidence; use a separate clean clone instead. Exploratory
   Dev runs may use modified build inputs, but a release-gate run requires
   `git_dirty=false` in the helper manifest. This means the fingerprinted build
   inputs match the candidate commit. Record
   `git_head_sha`, which is the candidate commit rather than GitButler's
   synthetic workspace HEAD. The manifest is
   `${ANARLOG_QA_TARGET_DIR:-$HOME/Library/Caches/anarlog/native-dev-qa-target}/.anarlog-native-dev-qa-manifest`.
2. Require every Dev gate to pass, then trigger `desktop_cd.yaml` with
   `channel=staging` from a branch or ref whose tip is that exact SHA. Verify
   the Actions run's head SHA matches the manifest before testing it.
3. Download the artifact from that specific run:

   ```bash
   gh run download <run-id> --name hyprnote-staging-macos-silicon
   ```

   Do not use a “latest staging” download for release evidence. Record the
   DMG SHA-256, install it, and repeat the core gates: sign-in, note creation,
   full-fixture recording/AEC, transcript preservation, automated summary,
   and recording/chat CloudSync deferral.
4. Stable is allowed only when Dev and that exact staging artifact pass for
   the final `main` SHA, including its changelog. Verify `main` still points to
   that SHA before triggering stable. Any rebuild or source change invalidates
   the prior evidence.
5. After stable publishes, download the matching architecture DMG from the
   `desktop_v<version>` GitHub release, record its SHA-256, install it, and
   verify the app reports that version. Use Computer Use to repeat the core
   sign-in, note, full-fixture recording/AEC, transcript, summary, chat, and
   CloudSync gates against the installed stable app. Keep fixture playback in
   the terminal. Do not mark the release complete until this stable pass
   succeeds.

## CloudSync platform scope

The patched native CloudSync vendor bundle and its request-cancellation tests
currently cover only macOS Apple Silicon and Intel. This evidence must not be
used to approve Windows, Linux, or mobile.

Before enabling or releasing any of those lanes, rebuild every target
architecture's bundled CloudSync native library from the patched source and
run the equivalent cancellation/drain suite against that exact bundle. At
minimum, prove stalled send/receive and manual/legacy network calls, logout,
configuration cleanup/init, worker-idle fencing, and an immediate local write
after cancellation. Rust-level fail-closed behavior or passing macOS bundle
tests alone is not cross-platform evidence.

## Checklist

### 1. Sign in, callback handoff, and sign out

- Start signed out and initiate sign-in from the desktop app.
- PASS when: reaching the browser's signed-in callback page automatically
  opens the native-protocol prompt without a click; accepting it signs the
  desktop app in.
- Sign out, repeat the flow, dismiss the automatic prompt, then click the
  page's **Open Anarlog** button. PASS when the button opens the same native
  prompt and completes desktop sign-in without a duplicate or expired-link
  error.
- Sign out once more with CloudSync enabled. The app must return promptly to
  its signed-out state without a stuck spinner, SQLite lock, or orphaned sync
  request.

### 2. Calendar connect, events, notifications

- Settings → Calendar (or onboarding): connect Apple Calendar and/or
  Google/Outlook via the integration flow.
- PASS when: the calendar list renders the account's calendars, events for
  today/this week appear in the timeline/sidebar, and an upcoming-event
  notification (meeting-start notification or in-app banner) fires for a
  test event starting within the notification window.
- Also verify: toggling a calendar off hides its events; ignore/unignore
  on a timeline event sticks (no snap-back after rapid toggling).

### 3. Create a new note

- Create a note from the sidebar/new-note affordance.
- PASS when: the editor opens immediately (no blocking wait), typed
  content persists after switching notes and after app restart, and the
  note appears in the timeline.

### 4. Start a recording

- In the note, start listening/recording, then play the repo audio fixture
  from the terminal so the system-audio path receives the source directly
  while the built-in microphone also hears it through the MacBook speakers.
- PASS when: the recording starts without error, live transcript words
  appear (when live transcription is enabled for the provider), and the
  recording indicator/timer runs. Mute/unmute must not wedge the session.
- Also verify both microphone and system-audio inputs carry nonzero signal,
  AEC initializes without an error or fallback, and the transcript follows
  the podcast once rather than duplicating phrases from speaker leakage.
- `audio_mic.wav` is the post-AEC, post-VAD microphone track, not the raw
  microphone. For a playback-only run, require all of:
  - `audio_mic.wav` and `audio_spk.wav` are readable mono 16 kHz WAVs whose
    durations differ by less than 0.1 seconds.
  - RemoteParty has at least 500 transcript words.
  - DirectMic words are at most 10% of RemoteParty words.
  - At most 5% of DirectMic bigrams also appear on RemoteParty within one
    second.
  - Residual mic/speaker absolute correlation is at most 0.10 in every
    active 30-second sample, with median attenuation of at least 20 dB.
- Any duplicated-bigram failure blocks release even when the processed
  microphone is quieter than the system-audio track. If the result is
  ambiguous, repeat a 90-second baseline with `NO_AEC=1`; enabling AEC must
  reduce duplicate bigrams by at least 80% and processed-mic RMS by at least
  10 dB.
- When a person is available, speak a unique phrase once over the podcast.
  PASS when it appears on DirectMic and the surrounding podcast remains only
  on RemoteParty. This protects real double-talk instead of solving echo by
  suppressing all microphone speech.
- With `AUDIO_SYNC_PROBE=1`, require an `audio_sync_probe` event and no
  `aec_init_failed`, `aec_failed`, `audio_sync_probe_panicked`, dropped
  samples, or mic/speaker queue-overflow events in the app log.
- With CloudSync enabled, require `deferred_for_capture: true` for the whole
  active recording. The status control must show a static **Saved locally**
  state, while transcript rows continue growing in the local database.
  No CloudSync request may start after capture deferral is acknowledged. If
  the log contains a capture-drain timeout, allow the single operation that
  started before deferral to settle, record the baseline afterward, then
  compare `last_sync_at_ms` and the SQLite Cloud request log across at least
  two 30-second intervals. No later send, receive, or E2EE witness work may
  run during capture.
- After Stop settles, require `deferred_for_capture: false`, one prompt
  trailing sync, and no SQLite lock/error cluster. A staged native outbox
  batch must remain unsent during capture and flush only after Stop.
- For transcript-integrity regressions, let the full 15:02 fixture play.
  Capture transcript word count, text length, and content hash immediately
  before Stop, after Stop settles, and after app restart. Counts must never
  shrink; the settled post-stop hash must survive restart unchanged.

### 5. Chat and overlapping activity sync deferral

- With CloudSync enabled and no recording active, send a chat request that
  runs long enough to inspect status. Once its native lease is acknowledged,
  require `activity_paused: true`, `deferred_for_capture: false`, and a static
  **Saved locally** status. Streaming and SQLite persistence must remain
  immediate.
- No CloudSync send, receive, or E2EE witness request may start while the chat
  lease is held. Apply the same pre-existing-operation drain exception as the
  recording gate, then compare `last_sync_at_ms` and the SQLite Cloud request
  log until the assistant response and its SQLite writes settle.
- After the final chat persist plus the 750 ms trailing delay, require
  `activity_paused: false` and exactly one coalesced trailing sync cycle.
  Abort/error and regeneration paths must also release their leases.
- Run one overlap: start recording, then start chat, and stagger their
  completion. Ending the first activity must not resume sync; status remains
  **Saved locally** and network sync stays at zero. Only the final lease may
  resume CloudSync, with exactly one trailing sync cycle afterward.

### 6. Automated summary after recording

- Stop the recording.
- PASS when: an enhanced note/summary is generated automatically without
  manual triggering, the summary reflects the spoken content, and a title
  is generated for untitled notes. A transcript must be attached to the
  session.
- With CloudSync enabled, verify the final summary-and-title persistence
  acquires an `enhance` activity lease before its final database read. No
  CloudSync request may overlap those writes; release must eventually
  succeed after transient bridge failures, followed by one trailing sync.
- Capture the summary body hash after generation and again after app restart.
  The generated title and settled summary hash must remain unchanged.

### 7. Provider matrix — repeat steps 3–6 under each config

| Config | How to set |
| --- | --- |
| On-device | Settings → AI: select local STT model and local LLM; sign-out state is also worth one pass |
| API keys | Settings → AI: configure a custom provider with an API key for both STT (if supported) and LLM |
| Pro plan | Settings → AI: select Anarlog cloud (`hyprnote` provider) with a Pro/trialing account |

- PASS when: steps 3–6 behave identically in outcome under each config
  (transcript + chat + automated summary), with provider-appropriate quality.
- Watch for: feature-gate prompts appearing for entitled users, silent
  summary failures (check the AI task state), and stalled live
  transcription (watchdog should batch-repair from the recording after
  stop).

## Automation notes

- Prefer driving the app UI via the Browser/automation tooling available
  in the session; the Tauri webview is not reachable by the in-app
  Browser pane, so use screenshots/accessibility tooling or ask the user
  to perform mic-dependent steps.
- Fixture playback and stop timing must run from the terminal. A human is
  only needed for the optional double-talk phrase and OAuth consent screens;
  verify the results programmatically (transcript rows, summary documents,
  calendar events, and audio diagnostics).
- Useful signals: `sessions`, `transcripts`, and `session_documents`
  (kind = summary) tables via the app DB; console/log output from the
  dev server for stall-watchdog and enhance-task errors.

## Reporting

Produce a table: checklist item × provider config → PASS/FAIL with a
one-line note. Include the Dev manifest's Git SHA/dirty state, staging run
URL and head SHA, staging artifact SHA-256, stable release URL and artifact
SHA-256, app version, and any explicit waiver. Any FAIL or SHA mismatch blocks
release; file or fix before cutting.

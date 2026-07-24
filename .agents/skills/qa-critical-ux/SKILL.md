---
name: qa-critical-ux
description: QA-test the critical desktop user experience before a release — calendar connect + notifications, note creation, recording, and automated summaries across on-device, API-key, and Pro providers. Use before cutting a stable release, after changes to STT/enhance/calendar/billing flows, or when asked to "QA the app".
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

   The helper owns a clean, persistent Cargo cache under
   `~/Library/Caches/anarlog`; never point it at the repository's `target`
   directory or clone that directory into the QA cache. The repository cache
   can contain enough old, provenance-tracked proc-macro dylibs for macOS
   assessment to stall `rustc` for minutes. Native builds use the full Xcode
   toolchain explicitly so Swift/MLX can invoke the Metal compiler instead of
   inheriting Command Line Tools.

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
2. For a non-production channel, supply that target's public
   `VITE_APP_URL`, `VITE_API_URL`, `VITE_SUPABASE_URL`, and
   `VITE_SUPABASE_ANON_KEY` when building instead of using the Dev helper.
3. Sign in with a test account that has calendar access. For provider
   matrix runs you need: a Pro (or trialing) account, an API key for at
   least one cloud provider (e.g. OpenAI), and a downloaded local STT +
   LLM model pair.
4. Note the app version and the provider config under test in the report.
5. For macOS audio regression runs, leave the MacBook open and use its
   built-in speakers and microphone with no external audio device attached.
6. Play the 15-minute Lex Fridman fixture from a long-lived terminal
   command after recording starts:

   ```bash
   /usr/bin/afplay -v 0.7 -t 900 \
     "$PWD/crates/data/src/english_10/audio.mp3"
   ```

   Let it finish naturally, or stop it with Ctrl-C. Do not use QuickTime or
   Computer Use just to control fixture playback.

## Checklist

### 1. Calendar connect, events, notifications

- Settings → Calendar (or onboarding): connect Apple Calendar and/or
  Google/Outlook via the integration flow.
- PASS when: the calendar list renders the account's calendars, events for
  today/this week appear in the timeline/sidebar, and an upcoming-event
  notification (meeting-start notification or in-app banner) fires for a
  test event starting within the notification window.
- Also verify: toggling a calendar off hides its events; ignore/unignore
  on a timeline event sticks (no snap-back after rapid toggling).

### 2. Create a new note

- Create a note from the sidebar/new-note affordance.
- PASS when: the editor opens immediately (no blocking wait), typed
  content persists after switching notes and after app restart, and the
  note appears in the timeline.

### 3. Start a recording

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

### 4. Automated summary after recording

- Stop the recording.
- PASS when: an enhanced note/summary is generated automatically without
  manual triggering, the summary reflects the spoken content, and a title
  is generated for untitled notes. A transcript must be attached to the
  session.

### 5. Provider matrix — repeat steps 2–4 under each config

| Config | How to set |
| --- | --- |
| On-device | Settings → AI: select local STT model and local LLM; sign-out state is also worth one pass |
| API keys | Settings → AI: configure a custom provider with an API key for both STT (if supported) and LLM |
| Pro plan | Settings → AI: select Anarlog cloud (`hyprnote` provider) with a Pro/trialing account |

- PASS when: steps 2–4 behave identically in outcome under each config
  (transcript + automated summary), with provider-appropriate quality.
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
one-line note. Any FAIL blocks release; file or fix before cutting.

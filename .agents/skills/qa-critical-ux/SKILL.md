---
name: qa-critical-ux
description: QA Anarlog's critical Pro user journey when explicitly asked — onboarding, responsive launch, microphone and system-audio capture, and automated summaries.
---

# QA: Critical User Experience

This is a standalone QA workflow. Run it only when the user explicitly asks
for QA; a release request alone does not invoke it. QA results do not approve
or block a release.

Keep the QA scope focused on the Pro user journey, starting from onboarding:

1. The app launches and never hangs.
2. Onboarding completes from scratch: permissions, sign-in, provider setup.
3. A recording captures both microphone and system audio.
4. Stopping the recording produces an automated summary.

Everything else is outside this skill's scope. Do not expand the run because
more checks are imaginable.

## Out of Scope — Tracked in Linear Instead

If you hit a failure in one of these areas incidentally, record it as an
informational note against the ticket and keep the requested QA focused. Do
not run dedicated fixtures or matrices for them.

- AEC quality (echo leakage, residual-echo metrics, double-talk): `ANLG-98`
- Automatic speaker identification / voiceprints: `ANLG-222`
- Real-world capture across devices, rooms, and live participants: `ANLG-284`
- Auth callback handoff and sign-out edge cases: `ANLG-285`
- Calendar connect, events, notifications: `ANLG-286`
- CloudSync activity deferral, leases, transcript-integrity hashes: `ANLG-287`
- On-device STT/LLM provider matrix: `ANLG-288`

Real-device and real-participant evaluation belongs to `ANLG-284`, not to this
checklist.

## Setup

Build and launch an authenticated native Dev bundle:

```bash
.agents/skills/qa-critical-ux/scripts/run-native-dev-qa.sh
```

The helper builds against the currently deployed production public
configuration (`VITE_APP_URL`, `VITE_API_URL`, `VITE_SUPABASE_URL`,
`VITE_SUPABASE_ANON_KEY` must match production; local URLs cannot pass), uses
its own persistent Cargo cache under `~/Library/Caches/anarlog` (never the
repo `target` directory), and launches with `AUDIO_SYNC_PROBE=1` and
`LISTENER_DEBUG=1`. It launches the `.app` through LaunchServices so macOS
evaluates process-scoped permissions against the bundle identity. Do not run
the executable under `Contents/MacOS` directly. Reuse an already-current
bundle with `--launch-only`.

When QA targets a specific candidate, pin the build to that commit:

```bash
ANARLOG_QA_GIT_SHA=<candidate-commit-sha> \
  .agents/skills/qa-critical-ux/scripts/run-native-dev-qa.sh
```

In a GitButler workspace, identify the branch tip with `but status` and inspect
it with `but show <branch>`; `git rev-parse HEAD` is a synthetic workspace
commit and is not the candidate identity.

A freshly rebuilt Dev bundle can trigger a login-Keychain prompt for the E2EE
recovery key (its code-signing hash changed). Enter the password the user
supplied for the QA machine, click **Always Allow**, and never place the
password in commands, logs, screenshots, or files. `--launch-only` keeps the
same binary and avoids another prompt.

Note the app version in the report. For audio, leave the MacBook open on
built-in speakers and microphone with no external device attached; the
helper's preflight enforces this.

### Start from onboarding

When Dev or staging QA is requested from first launch, reset that channel
before the run:

1. Quit the app, then delete its app data so `app.db` is gone:

   ```bash
   rm -rf ~/Library/Application\ Support/com.hyprnote.dev      # Dev
   rm -rf ~/Library/Application\ Support/com.hyprnote.staging  # staging
   ```

2. Launch with the onboarding flag, which clears auth, settings, and the
   store and resets microphone, system-audio, screen-recording,
   accessibility, calendar, and reminders permission state (it does not
   touch `app.db` — that is why step 1 deletes the directory):

   ```bash
   ONBOARDING=1 .agents/skills/qa-critical-ux/scripts/run-native-dev-qa.sh --launch-only
   ```

   For the installed staging app:

   ```bash
   open -a "Anarlog Staging" --args --onboarding 1
   ```

3. Complete onboarding for real: grant each permission when prompted, sign in
   with the **Pro (or trialing)** test account, and select Anarlog cloud
   (`anarlog` provider) in Settings → AI.

The installed **stable** app is not reset. Run the stable pass against its
existing data — that is the update-in-place state real users are in.
From-scratch onboarding evidence comes from the Dev and staging passes.

## Checklist — the Pro user journey

### 1. Launch and stay responsive

- The app launches to a usable window without a hang, freeze, beachball, or
  "could not start" dialog. This is the single most important check.
- Quit and relaunch once: startup completes promptly again and the UI responds.
- The log shows no panic, deadlock, or repeated-error loop.

### 2. Complete onboarding as a Pro user

- On Dev and staging (after the reset above): onboarding walks through
  cleanly — each permission grant sticks, sign-in with the Pro test account
  completes, and the app lands in the signed-in, entitled state with no
  feature-gate prompts. A stall, dead button, or loop anywhere in onboarding
  is a FAIL.
- On stable: verify the existing signed-in, entitled state is intact after
  the update; do not reset.
- Deeper callback/sign-out permutations are `ANLG-285`, not this gate.

### 3. Create a note and record

- Create a note; the editor opens immediately and typed content persists.
- Start listening/recording. After it starts, play the bundled fixture from a
  terminal to exercise system audio:

  ```bash
  /usr/bin/afplay -v 0.7 -t 180 "$PWD/crates/data/src/english_10/audio.mp3"
  ```

- PASS when: recording starts without error, the indicator/timer runs, both
  microphone and system-audio inputs carry nonzero signal, live transcript
  words appear, and mute/unmute does not wedge the session.
- On the Dev pass launched by `run-native-dev-qa.sh` with
  `AUDIO_SYNC_PROBE=1`, the log must show an `audio_sync_probe` event;
  staging/stable launches via `open` do not emit it, so its absence there is
  not a failure. Every launch must have no `audio_sync_probe_panicked`,
  dropped-sample, or queue-overflow events.
- Echo leakage, duplicate phrases, and speaker naming are informational
  (`ANLG-98`, `ANLG-222`, `ANLG-284`). Generic speaker labels are acceptable.

### 4. Stop and get a summary

- Stop the recording. The app must not hang while settling.
- PASS when: an enhanced summary is generated automatically without manual
  triggering, the summary reflects the spoken content, a title is generated
  for the untitled note, and a transcript is attached to the session.
- Restart the app: the note, transcript, and summary are still there.

## Choose the Requested Channel

Run only the channel or channels the user requests. If no channel is
specified, default to the native Dev build and report staging and stable as
`NOT REQUESTED`.

- **Dev:** Run the checklist on the requested commit from a clean checkout
  (`git_dirty=false` in the helper manifest; `git_head_sha` is the candidate
  commit, not GitButler's synthetic HEAD). The manifest is
  `${ANARLOG_QA_TARGET_DIR:-$HOME/Library/Caches/anarlog/native-dev-qa-target-v2}/.anarlog-native-dev-qa-manifest`.
- **Staging:** Trigger `desktop_cd.yaml` with `channel=staging` from the
  requested SHA. Verify the Actions run's head SHA, download that run's
  artifact (`gh run download <run-id> --name
  hyprnote-staging-macos-silicon` — never a "latest staging" download), record
  the DMG SHA-256, install it, reset staging, and run from onboarding.
- **Stable:** When explicitly requested, download the named release DMG,
  record its SHA-256, install it, verify the reported version, and run against
  the existing stable data without resetting it.

Do not repurpose the Dev helper as a staging build; staging evidence must come
from the signed `desktop_cd.yaml` artifact.

## Automation notes

- The Tauri webview is not reachable by the in-app Browser pane; drive the app
  with screenshots/accessibility tooling (Computer Use).
- Run fixture playback and stop timing from the terminal, not QuickTime.
- Verify results programmatically where possible: `sessions`, `transcripts`,
  and `session_documents` (kind = summary) tables in the app DB, plus app
  logs for the probe/panic signals above.

## Reporting

Report the requested channel, candidate SHA, app version, applicable artifact
SHA-256s, and PASS/FAIL with one line of evidence for each of the four
checklist items. A FAIL or SHA mismatch means the QA run failed; it does not
automatically block a release. Out-of-scope failures go to their Linear ticket
as informational notes.

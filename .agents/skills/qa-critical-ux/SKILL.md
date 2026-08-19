---
name: qa-critical-ux
description: QA the critical Pro user journey before a desktop release — onboards from scratch, launches without hanging, captures microphone and system audio, and produces an automated summary. Use before cutting a stable release or when asked to QA the app.
---

# QA: Critical User Experience

The release gate is one thing: **the Pro user journey works, starting from
onboarding**.

1. The app launches and never hangs.
2. Onboarding completes from scratch: permissions, sign-in, provider setup.
3. A recording captures both microphone and system audio.
4. Stopping the recording produces an automated summary.

Everything else is explicitly not a gate. Do not expand the run because more
checks are imaginable.

## Not a gate — tracked in Linear instead

If you hit a failure in one of these areas incidentally, record it as an
informational note against the ticket and keep going. Do not patch the release
candidate, block publication, or run dedicated fixtures/matrices for them.

- AEC quality (echo leakage, residual-echo metrics, double-talk): `ANLG-98`
- Automatic speaker identification / voiceprints: `ANLG-222`
- Real-world capture across devices, rooms, and live participants: `ANLG-284`
- Auth callback handoff and sign-out edge cases: `ANLG-285`
- Calendar connect, events, notifications: `ANLG-286`
- CloudSync activity deferral, leases, transcript-integrity hashes: `ANLG-287`
- On-device STT/LLM provider matrix: `ANLG-288`

AEC and speaker identification return to being gates only when their issues
are completed; real-device and real-participant evaluation belongs to
`ANLG-284`, not to this checklist.

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
`LISTENER_DEBUG=1`. Reuse an already-current bundle with `--launch-only`.

Pin release-gate builds to the intended candidate commit:

```bash
ANARLOG_QA_GIT_SHA=<candidate-commit-sha> \
  .agents/skills/qa-critical-ux/scripts/run-native-dev-qa.sh
```

In a GitButler workspace, take the branch tip's full `commitId` from
`but status --format json`; `git rev-parse HEAD` is a synthetic workspace
commit and is not release provenance.

A freshly rebuilt Dev bundle can trigger a login-Keychain prompt for the E2EE
recovery key (its code-signing hash changed). Enter the password the user
supplied for the QA machine, click **Always Allow**, and never place the
password in commands, logs, screenshots, or files. `--launch-only` keeps the
same binary and avoids another prompt.

Note the app version in the report. For audio, leave the MacBook open on
built-in speakers and microphone with no external device attached; the
helper's preflight enforces this.

### Start from onboarding

Dev and staging runs begin at first launch, not in an already-configured app.
Reset the channel before the run:

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

## Release-candidate order

1. Run the checklist on a Dev build of the exact candidate SHA from a clean
   checkout (`git_dirty=false` in the helper manifest; `git_head_sha` is the
   candidate commit, not GitButler's synthetic HEAD). The manifest is
   `${ANARLOG_QA_TARGET_DIR:-$HOME/Library/Caches/anarlog/native-dev-qa-target-v2}/.anarlog-native-dev-qa-manifest`.
2. Trigger `desktop_cd.yaml` with `channel=staging` from that exact SHA and
   verify the Actions run's head SHA matches the manifest. Download that
   run's artifact (`gh run download <run-id> --name
   hyprnote-staging-macos-silicon` — never a "latest staging" download),
   record the DMG SHA-256, install it, reset the staging channel, and repeat
   the checklist from onboarding.
3. Stable is allowed only when Dev and that exact staging artifact pass for
   the final `main` SHA. After stable publishes, download the release DMG,
   record its SHA-256, install, verify the reported version, and run the
   checklist once more against the existing stable data (no reset).

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

Report the candidate SHA, app version, artifact SHA-256s (staging/stable when
applicable), and PASS/FAIL with one line of evidence for each of the four
checklist items. Any FAIL or SHA mismatch blocks release. Failures in
non-gate areas go to their Linear ticket as informational notes.

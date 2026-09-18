---
name: qa-critical-ux
description: QA BlackMushi's critical Pro user journey on a signed staging candidate — onboarding, responsive launch, microphone and system-audio capture, automated summaries, and cloud sync.
---

# QA: Critical User Experience

Run this workflow only when the user explicitly asks for QA. A release request alone does not invoke it, and QA results do not approve or block a release.

Test only the signed staging artifact for the requested candidate:

1. The app launches and never hangs.
2. Onboarding completes from scratch: permissions, sign-in, and provider setup.
3. A recording captures both microphone and system audio.
4. Stopping the recording produces an automated summary.
5. The note syncs to the account and comes back after a local wipe.

Everything else is outside this skill's scope.

## Out of Scope

Record incidental failures against the existing Linear ticket without expanding this run:

- AEC quality: `ANLG-98`
- Automatic speaker identification: `ANLG-222`
- Real-world capture across devices, rooms, and participants: `ANLG-284`
- Auth callback and sign-out edge cases: `ANLG-285`
- Calendar, events, and notifications: `ANLG-286`
- CloudSync activity deferral and transcript integrity: `ANLG-287`
- On-device STT/LLM provider matrix: `ANLG-288`

## Build the Staging Candidate

Trigger `desktop_cd.yaml` with `channel=staging` from the requested candidate SHA. Verify the run's head SHA, download that run's artifact by ID, and never use a latest-staging download:

```bash
gh workflow run desktop_cd.yaml \
  --ref <candidate-ref> \
  -f channel=staging \
  -f candidate_sha=<40-character-candidate-sha>
gh run view <run-id> --json headSha,url
gh run download <run-id> --name hyprnote-staging-macos-silicon
```

Record the run URL and DMG SHA-256, install the artifact, and verify the app reports the expected version. The staging artifact must be signed output from that exact workflow run; do not substitute a local build.

Leave the MacBook open on its built-in speakers and microphone with no external audio device attached.

## Start from Onboarding

1. Fully quit BlackMushi Staging.
2. Reset staging permissions while the app is closed:

   ```bash
   .agents/skills/qa-critical-ux/scripts/reset-native-qa-permissions.sh
   ```

3. Back up any needed staging data, then remove the staging application data:

   ```bash
   rm -rf ~/Library/Application\ Support/com.hyprnote.staging
   ```

4. Launch through LaunchServices without an onboarding override:

   ```bash
   open -a "BlackMushi Staging" --env ONBOARDING=
   ```

Missing app data starts onboarding normally. Do not use `ONBOARDING=1`; resetting permissions asynchronously after initialization can suppress the microphone prompt. Do not edit permission databases.

Complete onboarding for real: grant each permission, sign in with the Pro or trialing test account, select BlackMushi cloud (`anarlog`) in Settings → AI, and turn on encrypted cloud sync in Settings → Sync. Use that account's existing recovery key when prompted. Creating a new key on an account that already has sync fails this item.

## Checklist

### 1. Launch and stay responsive

- The app opens to a usable window without a hang, freeze, beachball, or startup error.
- Quit and relaunch once; startup completes and the UI remains responsive.
- Logs contain no panic, deadlock, or repeated-error loop.

### 2. Complete onboarding as a Pro user

- Each permission prompt appears and the grant persists.
- Sign-in completes and the app reaches the entitled state without feature-gate prompts.
- Settings → Sync turns on with the account's existing recovery key.
- A stalled, dead, or looping onboarding step fails this item.

### 3. Create a note and record

- Create a note, type content, and verify it persists.
- Start recording, then play the bundled fixture from the terminal:

  ```bash
  /usr/bin/afplay -v 0.7 -t 180 "$PWD/crates/data/src/english_10/audio.mp3"
  ```

- The timer runs, microphone and system-audio inputs have nonzero signal, live transcript words appear, and mute/unmute does not wedge the session.
- Logs contain no `audio_sync_probe_panicked`, dropped-sample, or queue-overflow events.
- Echo leakage, duplicate phrases, and generic speaker labels are informational under the out-of-scope tickets.

### 4. Stop and get a summary

- Stopping does not hang while settling.
- An enhanced summary and title are generated automatically, reflect the fixture content, and remain attached with the transcript.
- Restart the app and verify the note, transcript, and summary persist.

### 5. Sync the note and restore it

- After the summary exists, open Settings → Sync. Status must reach **Synced**, not stay on Connecting, Syncing, Saved locally, or Sync needs attention.
- Record the note title shown in the app. Quit BlackMushi Staging. Do not reset permissions again. Wipe staging data once more:

  ```bash
  rm -rf ~/Library/Application\ Support/com.hyprnote.staging
  ```

- Launch through LaunchServices, sign in with the same Pro or trialing account, and enter the same recovery key.
- The same note, transcript, and summary reappear after restore. A missing note, empty transcript, or missing summary fails this item.
- Mid-recording deferral, chat/enhance leases, and hash-stable transcript integrity stay informational under `ANLG-287`.

Verify results programmatically where possible through the app's supported interfaces and logs. Do not query the app database directly.

## Reporting

Report:

- candidate SHA and staging workflow URL
- app version and DMG SHA-256
- PASS or FAIL for each checklist item, with one line of evidence
- out-of-scope observations separately

A failure or SHA mismatch fails the QA run but does not automatically block a release.

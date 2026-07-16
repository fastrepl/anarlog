# Windows release gate

This is the canonical manual QA checklist for
[ANLG-130](https://linear.app/fastrepl-inc/issue/ANLG-130/create-windows-manual-qa-matrix-and-release-gate).
It gates the first Windows release tracked by
[ANLG-68](https://linear.app/fastrepl-inc/issue/ANLG-68/ship-windows-desktop-capture-notifications-and-overlay-support).

Use the exact candidate artifact in every required run. A source build, VM run, or
remote-desktop run can find defects, but it does not replace a required physical-hardware
run.

## Status values

Use one of these values in every result cell:

- PASS: the stated pass criteria were observed and evidence is linked.
- FAIL: the behavior was exercised and did not meet the pass criteria.
- BLOCKED: the test could not start because a prerequisite is missing or broken.
- NOT RUN: the test has not been attempted for this candidate.
- DEFERRED: product scope explicitly excludes the behavior from this release.
- NOT SUPPORTED: the release explicitly does not support this platform or architecture.

Do not use PASS for a code review, successful compilation, or a result inherited from a
different artifact hash.

## Current candidate

| Field                | Value                                |
| -------------------- | ------------------------------------ |
| Version              | TBD                                  |
| Commit               | TBD                                  |
| Installer artifact   | TBD                                  |
| Installer SHA-256    | TBD                                  |
| Updater artifact     | TBD                                  |
| Updater SHA-256      | TBD                                  |
| Test window          | TBD                                  |
| QA owner             | TBD                                  |
| Decision             | NOT EVALUATED                        |
| Decision rationale   | Required runs have not been recorded |
| Blocking issue links | None recorded                        |

## Release coverage

### Architecture and hardware

| Cell ID            | Environment                                      | Release role                                                              | Required for the first release | Current status | Evidence |
| ------------------ | ------------------------------------------------ | ------------------------------------------------------------------------- | ------------------------------ | -------------- | -------- |
| W-ENV-X64-AMD      | Physical Windows 11 x64 on AMD                    | Primary real-hardware release gate                                        | Yes                            | NOT RUN        | TBD      |
| W-ENV-X64-INTEL    | Physical Windows 11 x64 on Intel                  | Additional x64 confidence; status must be explicit                        | No independent block          | NOT RUN        | TBD      |
| W-ENV-X64-CLEAN    | Clean Windows 11 x64 local machine or local VM    | Installer, SmartScreen, launch, updater, and uninstall                    | Yes                            | NOT RUN        | TBD      |
| W-ENV-ARM-EMU      | Windows 11 ARM running the published x64 artifact | Optional x64-emulation install, launch, auth, SQLite, CloudSync, and recording smoke | No                             | NOT RUN        | TBD      |
| W-ENV-ARM-PHYSICAL | Physical Windows ARM                              | Community confidence only                                                 | No                             | NOT RUN        | TBD      |
| W-ENV-ARM64-NATIVE | Native Windows ARM64 artifact                     | Not supported until a native CloudSync DLL and release artifact exist     | No                             | NOT SUPPORTED  | TBD      |

AMD and Intel are both Windows x64. A passing AMD run is the initial physical x64 gate;
Intel remains a recorded confidence cell, not a separate architecture. Native ARM64 is a
different target. The repository currently bundles CloudSync only for
windows/x86_64.

### Audio and meeting applications

The required rows provide pairwise coverage without requiring every device to be tested
against every meeting application. Run the device-change and lifecycle tests with the
same devices after these rows pass.

| Test ID  | Input                     | Output                     | Application              | Required | Current status | Evidence |
| -------- | ------------------------- | -------------------------- | ------------------------ | -------- | -------------- | -------- |
| W-AUD-01 | Built-in microphone       | Built-in speakers          | Zoom                     | Yes      | NOT RUN        | TBD      |
| W-AUD-02 | Bluetooth headset         | Bluetooth headset          | Google Meet in a browser | Yes      | NOT RUN        | TBD      |
| W-AUD-03 | USB microphone            | Wired or USB headphones    | Slack huddle or call     | Yes      | NOT RUN        | TBD      |
| W-AUD-04 | Any already-passing input | Any already-passing output | Microsoft Teams          | No       | NOT RUN        | TBD      |

If the machine has no built-in audio, record the OEM configuration and substitute a
second independent USB or wired device. Do not silently drop a required device class.

### Displays

| Test ID  | Display configuration                         | Required    | Current status | Evidence |
| -------- | --------------------------------------------- | ----------- | -------------- | -------- |
| W-DSP-01 | Single display                                | Yes         | NOT RUN        | TBD      |
| W-DSP-02 | Internal plus external display                | Yes         | NOT RUN        | TBD      |
| W-DSP-03 | Mixed-DPI displays, when hardware supports it | Conditional | NOT RUN        | TBD      |

If mixed-DPI hardware is unavailable, mark W-DSP-03 DEFERRED, record the gap in the
release notes, and do not claim mixed-DPI overlay support.

## Required test checklist

### Artifact, install, update, and removal

| Test ID    | Required | Procedure                                                                                              | Pass criteria                                                                                                                               | Result and evidence |
| ---------- | -------- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- |
| W-ART-01   | Yes      | Record the candidate URL, filename, version, commit, size, and SHA-256.                                | The hash matches the published checksum and every required run uses that exact hash.                                                        | NOT RUN             |
| W-ART-02   | Yes      | Inspect the executable and installer Authenticode signatures.                                         | Signature status is valid, the publisher is Fastrepl's approved publisher, and timestamp verification succeeds.                            | NOT RUN             |
| W-ART-03   | Yes      | Confirm the Tauri updater signature is published separately from the Authenticode signature.          | The updater artifact and its .sig file are present and tied to the candidate version.                                                       | NOT RUN             |
| W-INS-01   | Yes      | Install on W-ENV-X64-CLEAN from a fresh download. Record every SmartScreen and UAC screen.             | Publisher identity is correct, any reputation warning is documented verbatim, installation completes, and the app launches.                | NOT RUN             |
| W-INS-02   | Yes      | Check install location, Start menu entry, app icon, protocol registration, and single-instance focus. | Entries use the Anarlog identity; hyprnote:// opens the installed app; a second launch focuses the existing instance.                       | NOT RUN             |
| W-UPD-01   | Yes      | Install the previous supported version, create local data, then update to the candidate.               | Update detection, download, signature verification, install, restart, version change, and existing data all succeed.                       | NOT RUN             |
| W-UPD-02   | Yes      | Interrupt or reject one update attempt, then retry normally.                                           | The installed version remains launchable after the rejected attempt and a later retry succeeds without data loss.                          | NOT RUN             |
| W-UNINS-01 | Yes      | Uninstall from Windows Settings, then reinstall the same candidate.                                    | Executables and shortcuts are removed; user data is neither unexpectedly deleted nor duplicated; reinstall opens the expected local data. | NOT RUN             |

### Core application, credentials, and sync

| Test ID   | Required | Procedure                                                                                                      | Pass criteria                                                                                                                              | Result and evidence |
| --------- | -------- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | ------------------- |
| W-CORE-01 | Yes      | Launch, sign in, reach the main shell, open settings, create a session, edit its title and notes, then restart. | No platform-only crash occurs; the session and edits remain present after restart.                                                         | NOT RUN             |
| W-CORE-02 | Yes      | Lock and unlock Windows, minimize and restore the app, then reboot and launch from the Start menu.             | Main-window state is usable after each transition and no duplicate background instance remains.                                            | NOT RUN             |
| W-CRED-01 | Yes      | Save and update a provider secret through the app, restart twice, and use the provider after each restart.     | The secret remains usable and a Windows Credential Manager entry exists under the Anarlog secure-store service without plaintext fallback. | NOT RUN             |
| W-CRED-02 | Yes      | Sign out and back in, including one canceled sign-in, then repeat the provider and sync checks.                | Auth state follows the sign-in lifecycle, durable provider secrets follow declared product policy, and existing secrets are not corrupted. | NOT RUN             |
| W-SYNC-01 | Yes      | Enable CloudSync, create and edit a session, observe it on a second client, restart Windows, and edit again.    | Sync completes in both directions before and after restart without duplicate or missing sessions.                                          | NOT RUN             |
| W-SYNC-02 | Yes      | Start once without network, edit existing local data, restore network, and trigger or wait for sync.           | The app remains usable offline and later syncs without losing the offline edit.                                                             | NOT RUN             |

Do not attach app.db or credential exports as evidence. Record only file metadata and
observable behavior.

### Audio capture

For W-AUD-01 through W-AUD-03, play remote speech and speak locally for at least 30
seconds. Inspect the saved recording or transcript and the log markers. A visual level
meter alone is not proof.

| Test ID  | Required | Procedure                                                                                               | Pass criteria                                                                                                                            | Result and evidence |
| -------- | -------- | ------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------- |
| W-AUD-01 | Yes      | Run the built-in audio and Zoom matrix row.                                                             | Microphone and WASAPI system audio are both non-silent, concurrent, intelligible, and assigned to the expected channels.                 | NOT RUN             |
| W-AUD-02 | Yes      | Run the Bluetooth and Meet matrix row.                                                                  | Both streams remain non-silent and usable with the Bluetooth profile selected by Windows.                                                | NOT RUN             |
| W-AUD-03 | Yes      | Run the USB microphone and Slack matrix row.                                                            | Both streams remain non-silent and usable without stalls or persistent drift.                                                            | NOT RUN             |
| W-AUD-05 | Yes      | Start capture before meeting audio, stop, then start capture after meeting audio is already playing.    | Both start orders initialize microphone and WASAPI loopback and produce usable recordings.                                               | NOT RUN             |
| W-AUD-06 | Yes      | Start and stop ten recordings in succession.                                                            | All ten recordings finalize, no source remains stuck, and the next recording starts normally.                                            | NOT RUN             |
| W-AUD-07 | Yes      | Change the default input and output while recording, then hot-plug the Bluetooth and USB devices.       | Capture recovers automatically or stops with a visible actionable error; the app can start a new good recording without restart.         | NOT RUN             |
| W-AUD-08 | Yes      | Suspend and resume Windows during an active recording, then start a new recording.                      | The interrupted recording is not silently presented as complete; the app recovers or reports an actionable error and can record again.  | NOT RUN             |
| W-AUD-09 | Yes      | Record continuously for 60 minutes with microphone and system audio active.                            | Both streams remain present through the end, finalization succeeds, and logs show no sustained queue overflow or capture-thread failure. | NOT RUN             |
| W-AUD-10 | Yes      | Change Windows sample rate for one endpoint between recordings and repeat a short concurrent capture. | The next recording uses the new configuration or shows a clear unsupported-state error; output remains playable.                        | NOT RUN             |

### Notifications, overlay, detection, and desktop lifecycle

| Test ID   | Required    | Procedure                                                                                                           | Pass criteria                                                                                                                                | Result and evidence |
| --------- | ----------- | ------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- |
| W-NOT-01  | Yes         | Trigger meeting-detected and reminder notifications with Anarlog focused, minimized, and backgrounded.              | Each required notification appears once with correct title, body, icon, and app identity; click and dismiss behavior match product intent.  | NOT RUN             |
| W-NOT-02  | Yes         | Repeat one notification with Windows Do Not Disturb or Focus Assist off and on, then clear notifications.           | Suppression behavior follows Windows settings, no silent app error occurs, and clear removes Anarlog notifications.                         | NOT RUN             |
| W-OVR-01  | Yes         | Start a recording on a single display; move, interact with, and stop from the floating bar.                         | The bar is visible, always on top without stealing focus, controls emit once, and all overlay state closes after stop.                      | NOT RUN             |
| W-OVR-02  | Yes         | Move the recording window and floating bar between internal and external displays, minimize, restore, and Alt-Tab. | Position, scale, drag behavior, and controls remain usable and the bar does not become stranded off-screen.                                 | NOT RUN             |
| W-OVR-03  | Conditional | Repeat W-OVR-02 across mixed-DPI displays and after changing display scale.                                        | The overlay rescales and remains inside the active display bounds without requiring an app restart.                                        | NOT RUN             |
| W-DET-01  | Yes         | Start and end calls in Zoom, Meet, and Slack; observe meeting detection and microphone-using application state.     | Each supported app is identified without persistent helper-process noise; start/end transitions do not duplicate.                          | NOT RUN             |
| W-DET-02  | Yes         | Ignore one supported app, restore it, and exercise background and foreground browser tabs.                         | Ignore/include settings change behavior predictably and unsupported detection cases fail quietly with diagnostics.                         | NOT RUN             |
| W-PERM-01 | Yes         | Review onboarding and settings before and after microphone/system-audio use.                                       | Windows copy describes capability state accurately and does not promise a macOS-style permission prompt.                                   | NOT RUN             |
| W-DESK-01 | Yes         | Exercise tray show/hide, autostart, global shortcut, deep link, and second-instance activation.                    | Every advertised integration works after restart or is visibly gated before release.                                                        | NOT RUN             |
| W-UIA-01  | No          | If structured Windows UI Automation context is advertised, inspect supported meeting apps and capture evidence.    | Advertised fields are stable and accurate; otherwise the feature is absent from UI and release claims.                                      | DEFERRED            |

W-NOT-01 and W-NOT-02 depend on ANLG-126. W-OVR-01 through W-OVR-03 depend on
ANLG-127. W-DET-01 and W-DET-02 depend on ANLG-128. W-UIA-01 belongs to
ANLG-129 and is not a v1 release gate unless the feature is advertised.

## Evidence collection

Create a run ID such as W-RUN-20260717-AMD-01. Attach evidence to ANLG-130 or a
linked issue and put the attachment link in the tables above. Do not commit logs,
screenshots, recordings, database files, or credential exports to this repository.

Before attaching evidence:

- Remove access tokens, email addresses, meeting titles, participant names, and transcript
  content.
- Keep timestamps, device names, driver names, application versions, error messages, and
  the candidate hash.
- Record whether the run was local physical hardware, a local VM, ARM x64 emulation, or
  remote desktop.

### Environment metadata

Run in PowerShell:

~~~powershell
$run = "W-RUN-YYYYMMDD-01"
$evidence = Join-Path (Resolve-Path ".") "evidence\$run"
New-Item -ItemType Directory -Force $evidence | Out-Null

Get-ComputerInfo |
  Select-Object WindowsProductName, WindowsVersion, OsBuildNumber,
    CsManufacturer, CsModel, CsSystemType |
  Format-List |
  Out-File (Join-Path $evidence "computer.txt")

Get-CimInstance Win32_Processor |
  Select-Object Name, Manufacturer, Architecture |
  Format-List |
  Out-File (Join-Path $evidence "cpu.txt")

Get-PnpDevice -Class AudioEndpoint |
  Select-Object Status, FriendlyName, InstanceId |
  Format-Table -AutoSize |
  Out-File (Join-Path $evidence "audio-endpoints.txt")

Get-CimInstance Win32_SoundDevice |
  Select-Object Name, Manufacturer, Status, PNPDeviceID |
  Format-List |
  Out-File (Join-Path $evidence "sound-devices.txt")

Get-CimInstance Win32_VideoController |
  Select-Object Name, DriverVersion, CurrentHorizontalResolution,
    CurrentVerticalResolution |
  Format-List |
  Out-File (Join-Path $evidence "display.txt")
~~~

Record Windows display-scale percentages manually beside display.txt because the WMI
values above do not prove per-display DPI.

### Artifact and signature

~~~powershell
$artifact = Resolve-Path "PATH_TO_INSTALLER"

Get-FileHash -Algorithm SHA256 $artifact |
  Format-List

Get-AuthenticodeSignature $artifact |
  Select-Object Status, StatusMessage, Path,
    @{Name="Subject"; Expression={$_.SignerCertificate.Subject}},
    @{Name="Thumbprint"; Expression={$_.SignerCertificate.Thumbprint}},
    @{Name="TimestampSubject"; Expression={$_.TimeStamperCertificate.Subject}} |
  Format-List

signtool verify /pa /v $artifact
~~~

Get-AuthenticodeSignature and signtool verify the Windows Authenticode signature.
They do not verify the separate Tauri updater .sig file.

### Application logs and diagnostics

The stable bundle identifier is com.hyprnote.stable. The tracing plugin writes
app.log and up to five rotated files, app.log.1 through app.log.5, under the
Tauri app log directory.

~~~powershell
$bundleId = "com.hyprnote.stable"
$logDir = Join-Path $env:LOCALAPPDATA "$bundleId\logs"

Get-ChildItem $logDir -Filter "app.log*" |
  Select-Object FullName, Length, LastWriteTime

Get-Content (Join-Path $logDir "app.log") -Tail 500

Select-String -Path (Join-Path $logDir "app.log*") -Pattern "mic_input_initialized|wasapi_loopback_initialized|mic_stream_error|queue_overflow|capture_stream_failed"
~~~

Use com.hyprnote.staging for a staging package and com.hyprnote.dev for a dev
build. The stable app database normally lives at
$env:APPDATA\anarlog\app.db, but a migrated install can intentionally retain the
legacy hyprnote or bundle-identifier directory.

~~~powershell
$dbCandidates = @(
  (Join-Path $env:APPDATA "anarlog\app.db"),
  (Join-Path $env:APPDATA "hyprnote\app.db"),
  (Join-Path $env:APPDATA "com.hyprnote.stable\app.db")
)

$dbCandidates |
  Where-Object { Test-Path $_ } |
  ForEach-Object { Get-Item $_ } |
  Select-Object FullName, Length, LastWriteTime

Get-ChildItem (Join-Path $env:LOCALAPPDATA "char\cloudsync") -Recurse -Filter "cloudsync.dll" -ErrorAction SilentlyContinue |
  Select-Object FullName, Length, LastWriteTime

cmdkey /list |
  Select-String "com.anarlog.stable.secure-store"
~~~

The Credential Manager command records only target metadata. Never export or print the
credential value. The CloudSync cache path intentionally still uses char/cloudsync.

For an application crash, also collect a bounded Windows event-log slice:

~~~powershell
$since = (Get-Date).AddMinutes(-30)
Get-WinEvent -FilterHashtable @{LogName="Application"; StartTime=$since} |
  Where-Object { $_.Message -match "anarlog|hyprnote" } |
  Select-Object TimeCreated, ProviderName, Id, LevelDisplayName, Message |
  Format-List
~~~

### Optional source-build smoke

These commands are useful before packaging, but they do not replace tests of the signed
candidate artifact:

~~~powershell
pnpm -F desktop typecheck
cargo check -p desktop --target x86_64-pc-windows-msvc
pnpm -F desktop tauri:dev
$env:POSTHOG_API_KEY = "phc_local_smoke"
$env:VITE_API_URL = "https://api.anarlog.so"
pnpm -F desktop tauri build --ci --bundles nsis --no-sign --target x86_64-pc-windows-msvc --config ./src-tauri/tauri.conf.staging.json --features devtools
~~~

## Failure record template

Copy this section for each failure:

| Field                 | Value                                 |
| --------------------- | ------------------------------------- |
| Test ID               | W-XXX-00                              |
| Run ID                | W-RUN-YYYYMMDD-00                     |
| Candidate hash        | TBD                                   |
| Severity              | Release blocker / high / medium / low |
| Environment cell      | W-ENV-...                             |
| Device and driver     | TBD                                   |
| Meeting application   | TBD                                   |
| Display configuration | TBD                                   |
| Started at            | ISO-8601 timestamp with timezone      |
| Expected              | TBD                                   |
| Observed              | TBD                                   |
| Reproduction steps    | TBD                                   |
| Log excerpt or link   | Redacted attachment link              |
| Screenshot/video link | Redacted attachment link              |
| Tracking issue        | TBD                                   |
| Retest result         | NOT RUN                               |

## Ship and no-ship rules

Mark the candidate SHIP only when all of the following are true:

- Every required test is PASS for the exact published artifact hash.
- W-ENV-X64-AMD and W-ENV-X64-CLEAN pass their assigned gates.
- Intel x64, Windows ARM emulation, and physical ARM are explicitly labeled TESTED,
  UNTESTED, or NOT SUPPORTED in the current decision.
- Authenticode, updater signatures, published checksums, install, update, and uninstall
  pass.
- Microphone and WASAPI system audio are non-silent, concurrent, recoverable, and stable
  on physical x64 hardware.
- Credential Manager persistence, local SQLite durability, and CloudSync lifecycle tests
  pass.
- Notifications, recording controls, meeting detection, and the declared overlay scope
  pass.
- No open release-blocking issue remains.

Mark the candidate NO SHIP when any of these conditions is present:

- The artifact is unsigned, has an invalid signature, has a mismatched checksum, or is not
  the artifact that was tested.
- Install, launch, update, or uninstall can corrupt or unexpectedly remove user data.
- Microphone or system audio is silent, stalls, drifts beyond usable output, or cannot
  recover or fail visibly after a device change.
- Credentials disappear unexpectedly, are written to plaintext, or CloudSync loses or
  duplicates user data.
- A required notification, recording control, meeting-detection path, or declared overlay
  behavior is absent or silently no-ops.
- A platform capability is advertised even though it is unavailable.

The first Windows release may defer all of the following if the product and release notes
say so explicitly:

- A native Windows ARM64 artifact and native ARM64 CloudSync.
- Physical Windows ARM validation.
- Structured Windows UI Automation meeting context.
- Microsoft Teams-specific confidence coverage.
- Mixed-DPI overlay support when no suitable test hardware is available.
- Optional live-caption or overlay settings beyond the declared W-OVR scope.

It may not defer signed x64 artifacts, clean install/update/uninstall, physical x64 mic
and system-audio capture, durable local data, secure credentials, CloudSync, or the
desktop behaviors advertised to Windows users.

## Run ledger

Add one row per completed run. Keep detailed evidence in the linked attachment rather
than committing it to the repository.

| Run ID | Date | Candidate SHA-256 | Environment cell | Scope | Result  | Evidence | Tester |
| ------ | ---- | ----------------- | ---------------- | ----- | ------- | -------- | ------ |
| TBD    | TBD  | TBD               | TBD              | TBD   | NOT RUN | TBD      | TBD    |

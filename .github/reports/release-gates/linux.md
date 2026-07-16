# Linux beta release gate

This is the canonical manual QA checklist for
[ANLG-170](https://linear.app/fastrepl-inc/issue/ANLG-170/create-linux-manual-qa-matrix-and-beta-release-gate).
It gates the Linux beta tracked by
[ANLG-69](https://linear.app/fastrepl-inc/issue/ANLG-69/ship-linux-desktop-beta-with-system-audio-credentials-and-packaging).

The initial runtime baseline is Ubuntu 24.04 with GNOME, Wayland, and PipeWire.
Virtual machines cover build, package, UI, credential, updater, and virtual-audio smoke.
They do not replace physical or trusted-community runs for real audio routing,
Bluetooth/USB behavior, hot-plugging, suspend/resume, or desktop integration.

## Status values

Use one of these values in every result cell:

- PASS: the stated pass criteria were observed and evidence is linked.
- FAIL: the behavior was exercised and did not meet the pass criteria.
- BLOCKED: the test could not start because a prerequisite is missing or broken.
- NOT RUN: the test has not been attempted for this candidate.
- DEFERRED: product scope explicitly excludes the behavior from this beta.
- NOT SUPPORTED: the beta explicitly does not support this platform, package, or
  environment.

Do not use PASS for a code review, successful compilation, or a result inherited from a
different artifact hash.

## Current candidate

| Field                 | Value                                |
| --------------------- | ------------------------------------ |
| Version               | TBD                                  |
| Commit                | TBD                                  |
| x86_64 AppImage       | TBD                                  |
| x86_64 AppImage SHA-256 | TBD                                |
| x86_64 .deb           | TBD                                  |
| x86_64 .deb SHA-256   | TBD                                  |
| ARM64 artifact        | TBD                                  |
| ARM64 SHA-256         | TBD                                  |
| Test window           | TBD                                  |
| QA owner              | TBD                                  |
| Decision              | NOT EVALUATED                        |
| Decision rationale    | Required runs have not been recorded |
| Blocking issue links  | None recorded                        |

## Release coverage

### Runtime environments

| Cell ID             | Environment                                             | Release role                                                                      | Required | Current status | Evidence |
| ------------------- | ------------------------------------------------------- | --------------------------------------------------------------------------------- | -------- | -------------- | -------- |
| L-ENV-X64-PHYSICAL  | Physical or trusted-community Ubuntu 24.04 x86_64, GNOME/Wayland/PipeWire | Real x86_64 audio, credential, lifecycle, and desktop-integration gate | Yes      | NOT RUN        | TBD      |
| L-ENV-ARM64-PHYSICAL | Physical or trusted-community Ubuntu 24.04 ARM64, GNOME/Wayland/PipeWire | Real ARM64 audio, credential, lifecycle, and CloudSync gate            | Yes      | NOT RUN        | TBD      |
| L-ENV-ARM64-VM      | Full Ubuntu 24.04 ARM64 GNOME/Wayland VM                | Daily package, UI, credential, updater, and guest-audio smoke                      | Yes      | NOT RUN        | TBD      |
| L-ENV-X64-CLEAN     | Clean Ubuntu 24.04 x86_64 local machine or local VM     | AppImage and .deb clean install, upgrade, launch, and uninstall                    | Yes      | NOT RUN        | TBD      |
| L-ENV-KDE-WAYLAND   | KDE Plasma on Wayland                                   | Additional desktop confidence                                                      | No       | NOT RUN        | TBD      |
| L-ENV-GNOME-X11     | GNOME on X11                                            | Display-server fallback confidence                                                 | No       | NOT RUN        | TBD      |
| L-ENV-FEDORA        | Current Fedora release                                  | Additional distro confidence                                                       | No       | NOT RUN        | TBD      |

The x86_64 and ARM64 physical cells may be completed by trusted community testers, but
both must exist before the beta ships. A VM result must remain labeled as VM evidence.
Building release artifacts on Debian 12 or Ubuntu 22.04 for an older glibc baseline is a
CI policy; it is not a substitute for the Ubuntu 24.04 runtime cells above.

### Package matrix

| Package ID             | Architecture | Package   | Release role                                      | Required | Current status | Evidence |
| ---------------------- | ------------ | --------- | ------------------------------------------------- | -------- | -------------- | -------- |
| L-PKG-X64-APPIMAGE     | x86_64       | AppImage  | Primary portable beta package                     | Yes      | NOT RUN        | TBD      |
| L-PKG-X64-DEB          | x86_64       | .deb      | Primary Debian/Ubuntu beta package                 | Yes      | NOT RUN        | TBD      |
| L-PKG-ARM64-DEB        | ARM64        | .deb      | Required package for the ARM64 baseline run        | Yes      | NOT RUN        | TBD      |
| L-PKG-ARM64-APPIMAGE   | ARM64        | AppImage  | Required only if published or advertised           | Conditional | NOT RUN     | TBD      |
| L-PKG-FLATPAK          | Any          | Flatpak   | Separate sandbox gate tracked by ANLG-169          | No       | NOT SUPPORTED  | TBD      |

Do not publish an ARM64 filename that contains an x86_64 binary, or an x86_64 filename
that contains an ARM64 binary. Flatpak remains absent from downloads and release claims
until its dedicated audio, secret-storage, updater, and desktop-integration gate passes.

### Audio devices and meeting applications

The required rows provide pairwise application coverage. Across the two required physical
or community runs, the result set must also contain at least one built-in/default device,
one USB device, and one Bluetooth device. The three device classes do not each need to run
on both architectures.

| Test ID  | Environment              | Input and output                          | Application              | Required | Current status | Evidence |
| -------- | ------------------------ | ----------------------------------------- | ------------------------ | -------- | -------------- | -------- |
| L-AUD-01 | L-ENV-X64-PHYSICAL       | Default built-in or wired mic and output  | Zoom                     | Yes      | NOT RUN        | TBD      |
| L-AUD-02 | L-ENV-ARM64-PHYSICAL     | Default available mic and output          | Google Meet in a browser | Yes      | NOT RUN        | TBD      |
| L-AUD-03 | Either required physical cell | USB mic and wired or USB output       | Slack huddle or call     | Yes      | NOT RUN        | TBD      |
| L-AUD-04 | Either required physical cell | Bluetooth input and output             | Any required app         | Yes      | NOT RUN        | TBD      |
| L-AUD-05 | Any already-passing cell | Any already-passing input and output      | Teams or teams-for-linux | No       | NOT RUN        | TBD      |
| L-AUD-06 | L-ENV-ARM64-VM           | Guest virtual mic and output              | Browser playback         | Yes, smoke only | NOT RUN | TBD      |

## Required test checklist

### Artifacts, installation, upgrade, and removal

| Test ID     | Required | Procedure                                                                                                 | Pass criteria                                                                                                                               | Result and evidence |
| ----------- | -------- | --------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- |
| L-ART-01    | Yes      | Record the URL, filename, version, commit, size, and SHA-256 for every published Linux artifact.          | Hashes match published checksums and every required run uses an exact recorded hash.                                                        | NOT RUN             |
| L-ART-02    | Yes      | Inspect each artifact with file and inspect each .deb control record.                                     | Machine architecture, package Architecture, filename, download label, and updater target all agree.                                         | NOT RUN             |
| L-ART-03    | Yes      | Inspect AppImage and .deb contents for the expected binary, icon, desktop file, resources, and libraries. | The installed identity is Anarlog, the main binary is anarlog, and required shared libraries are declared or bundled correctly.             | NOT RUN             |
| L-INS-01    | Yes      | Launch the x86_64 AppImage on L-ENV-X64-CLEAN from a fresh download.                                      | It launches without an undeclared host dependency, reaches the main shell, and creates no duplicate app identity.                           | NOT RUN             |
| L-INS-02    | Yes      | Install the x86_64 .deb with apt on L-ENV-X64-CLEAN, then launch from the desktop and terminal.           | apt resolves declared dependencies, the launcher and icon work, and the app reaches the main shell.                                         | NOT RUN             |
| L-INS-03    | Yes      | Install and launch L-PKG-ARM64-DEB on the required ARM64 environment.                                     | The artifact is native ARM64, launches successfully, opens SQLite, and extracts the ARM64 CloudSync library.                                | NOT RUN             |
| L-UPD-01    | Yes      | Start from the prior AppImage release with local data, then use the advertised updater or replace it.     | The candidate launches with the existing data and the documented AppImage update path matches actual behavior.                             | NOT RUN             |
| L-UPD-02    | Yes      | Install the prior .deb, create local data, then install the candidate .deb with apt.                       | apt reports a successful upgrade, version changes, launcher remains valid, and local data survives.                                         | NOT RUN             |
| L-UNINS-01  | Yes      | Remove the .deb with apt remove, verify package files, then reinstall.                                    | Package-owned files are removed; user data is neither unexpectedly deleted nor duplicated; reinstall opens the expected data.              | NOT RUN             |
| L-UNINS-02  | Yes      | Remove the AppImage and any user-created launcher integration, then restore the same candidate.           | Removing the portable artifact leaves no broken advertised integration and restoring it reopens the expected user data.                    | NOT RUN             |

Do not use apt purge for L-UNINS-01. Purging is a separate destructive test and requires
an explicit data-backup plan.

### Core application, credentials, and CloudSync

| Test ID    | Required | Procedure                                                                                                         | Pass criteria                                                                                                                                 | Result and evidence |
| ---------- | -------- | ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- |
| L-CORE-01  | Yes      | Launch, sign in, reach the main shell, open settings, create a session, edit its title and notes, then restart.   | No platform-only crash occurs; the session and edits remain present after restart.                                                            | NOT RUN             |
| L-CORE-02  | Yes      | Lock and unlock the desktop, minimize and restore, log out and back in, then reboot.                              | The app relaunches normally after each transition and no duplicate or corrupted local state appears.                                          | NOT RUN             |
| L-CRED-01  | Yes      | With Secret Service unlocked, save, read, update, and delete a provider secret through normal app flows.         | Each operation succeeds through Secret Service and no secret appears in plaintext settings, app data, or logs.                                | NOT RUN             |
| L-CRED-02  | Yes      | Save a provider secret, then repeat its use after app restart, desktop lock/unlock, logout/login, and reboot.     | The secret remains usable through every required lifecycle transition on both required physical architecture cells.                           | NOT RUN             |
| L-CRED-03  | Yes      | Lock or make Secret Service unavailable, attempt a secret-backed action, restore the service, and retry.         | The unavailable state is actionable, existing credentials are not overwritten or lost, and retry succeeds after restoration.                | NOT RUN             |
| L-SYNC-01  | Yes      | Enable CloudSync, create and edit a session, observe it on a second client, restart Linux, and edit again.        | Sync completes in both directions before and after restart without duplicate or missing sessions on x86_64 and ARM64.                         | NOT RUN             |
| L-SYNC-02  | Yes      | Start once without network, edit existing local data, restore network, and trigger or wait for sync.             | The app remains usable offline and later syncs without losing the offline edit.                                                               | NOT RUN             |
| L-SYNC-03  | Yes      | Sign out, attempt an account mismatch, then sign back into the original account.                                  | Sync access is suspended or cleared according to product policy; another account cannot silently reuse the existing CloudSync workspace.      | NOT RUN             |

Do not attach app.db, Secret Service exports, or credential values as evidence. Record
only file metadata, service availability, and observable product behavior.

### Audio capture

For L-AUD-01 through L-AUD-04, play remote speech and speak locally for at least
30 seconds. Inspect the saved recording or transcript and the log markers. A visual level
meter alone is not proof.

| Test ID  | Required | Procedure                                                                                                            | Pass criteria                                                                                                                                 | Result and evidence |
| -------- | -------- | -------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- |
| L-AUD-01 | Yes      | Run the physical x86_64 and Zoom matrix row with the normal PipeWire path.                                           | Microphone and system audio are non-silent, concurrent, intelligible, and assigned to the expected channels.                                 | NOT RUN             |
| L-AUD-02 | Yes      | Run the physical or community ARM64 and Meet matrix row with the normal PipeWire path.                              | Microphone and system audio are non-silent and concurrent; the native ARM64 build remains stable.                                             | NOT RUN             |
| L-AUD-03 | Yes      | Run the USB and Slack matrix row on either required physical architecture.                                           | Both streams remain usable while the USB endpoints are selected and no persistent stall or drift appears.                                   | NOT RUN             |
| L-AUD-04 | Yes      | Run the Bluetooth matrix row on either required physical architecture.                                               | Both streams remain usable with the active Bluetooth profile; any unsupported profile produces a visible actionable state.                  | NOT RUN             |
| L-AUD-06 | Yes      | Run virtual microphone and playback capture inside L-ENV-ARM64-VM.                                                   | Basic guest audio is non-silent, but the result remains labeled as VM smoke rather than physical evidence.                                   | NOT RUN             |
| L-AUD-07 | Yes      | Force the PipeWire connection to fail for one disposable launch and record using the PulseAudio monitor fallback.   | Logs show pipewire_capture_unavailable followed by pulseaudio_capture_initialized, and the fallback recording is non-silent.                 | NOT RUN             |
| L-AUD-08 | Yes      | Start capture before meeting playback, stop, then start capture after playback is already running.                  | Both start orders initialize the microphone and a system-audio backend and produce usable recordings.                                        | NOT RUN             |
| L-AUD-09 | Yes      | Start and stop ten recordings, switch the default input and output, and hot-plug USB and Bluetooth devices.          | Recordings finalize; capture recovers or stops with a visible actionable error; a new good recording can start without desktop logout.       | NOT RUN             |
| L-AUD-10 | Yes      | Suspend and resume during an active recording, then start a new recording.                                           | The interrupted recording is not silently presented as complete; the app recovers or reports an actionable error and can record again.      | NOT RUN             |
| L-AUD-11 | Yes      | Record continuously for 60 minutes with microphone and system audio active.                                          | Both streams remain present through the end, finalization succeeds, and logs show no sustained queue overflow or capture-thread failure.     | NOT RUN             |
| L-AUD-12 | Yes      | Stop PipeWire, PulseAudio compatibility, or both in a disposable session, then open the relevant settings/onboarding view. | Missing audio services produce an accurate recoverable capability state instead of a macOS-style permission prompt or silent empty audio. | NOT RUN             |

### Permission UX and desktop integrations

| Test ID     | Required | Procedure                                                                                                           | Pass criteria                                                                                                                                | Result and evidence |
| ----------- | -------- | ------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- |
| L-PERM-01   | Yes      | Review onboarding and settings before and after microphone/system-audio use and with audio services unavailable.    | Copy describes runtime capability checks accurately and does not show permanently denied macOS Accessibility or Apple Calendar rows.        | NOT RUN             |
| L-PERM-02   | Yes      | Review local transcription/model choices on x86_64 and ARM64.                                                       | macOS-only or architecture-incompatible models cannot be selected and explain their availability accurately.                                | NOT RUN             |
| L-DESK-01   | Yes      | Start and end calls in Zoom, Meet, and Slack; observe meeting and microphone-use detection.                         | Each advertised app is identified without persistent helper-process noise and start/end transitions do not duplicate.                       | NOT RUN             |
| L-DESK-02   | Yes      | Trigger notifications focused, minimized, and backgrounded; click, dismiss, clear, and repeat once.                | Notifications appear once with correct identity and expected actions; clearing and main-window activation work.                              | NOT RUN             |
| L-DESK-03   | Yes      | Exercise tray show/hide and menu actions, then restart the desktop session.                                         | The tray remains usable on the declared GNOME baseline and does not strand the main window.                                                   | NOT RUN             |
| L-DESK-04   | Yes      | Enable autostart, reboot, open a hyprnote:// deep link, and launch a second instance.                                | Autostart behavior matches the setting, the deep link opens Anarlog, and the second launch focuses the existing instance.                    | NOT RUN             |
| L-DESK-05   | Conditional | Exercise every global shortcut advertised to Linux on GNOME/Wayland, including press, release, and repeated use. | Advertised shortcuts fire once without stuck state; otherwise the controls are visibly unavailable and absent from beta claims.              | NOT RUN             |
| L-DESK-06   | Conditional | Exercise dictation, floating controls, or live captions only if they are advertised to Linux.                   | Advertised controls are usable; unsupported controls are gated and cannot silently no-op.                                                    | NOT RUN             |

L-DESK-05 and L-DESK-06 may be DEFERRED only when the unavailable feature is hidden or
clearly disabled and is absent from beta claims. Package, audio, credentials, CloudSync,
permission accuracy, notification, tray, autostart, deep-link, and single-instance tests
remain required.

## Evidence collection

Create a run ID such as L-RUN-20260717-X64-01. Attach evidence to ANLG-170 or a
linked issue and put the attachment link in the tables above. Do not commit logs,
screenshots, recordings, database files, or credential exports to this repository.

Before attaching evidence:

- Remove access tokens, email addresses, meeting titles, participant names, and transcript
  content.
- Keep timestamps, distro and kernel versions, architecture, desktop/display server,
  audio-server versions, device and driver names, application versions, error messages,
  and candidate hashes.
- Record whether the run was physical hardware, trusted community hardware, a local VM,
  or remote desktop.

### Environment metadata

Run from a terminal inside the graphical login session:

~~~bash
run="L-RUN-YYYYMMDD-01"
evidence="$PWD/evidence/$run"
mkdir -p "$evidence"

uname -a > "$evidence/uname.txt"
uname -m > "$evidence/architecture.txt"
dpkg --print-architecture >> "$evidence/architecture.txt"
cat /etc/os-release > "$evidence/os-release.txt"
lscpu > "$evidence/cpu.txt"

printf "XDG_CURRENT_DESKTOP=%s\nXDG_SESSION_TYPE=%s\nXDG_SESSION_ID=%s\nWAYLAND_DISPLAY=%s\nDISPLAY=%s\n" \
  "$XDG_CURRENT_DESKTOP" "$XDG_SESSION_TYPE" "$XDG_SESSION_ID" \
  "$WAYLAND_DISPLAY" "$DISPLAY" > "$evidence/session.txt"

if [ -n "$XDG_SESSION_ID" ]; then
  loginctl show-session "$XDG_SESSION_ID" -p Type -p Desktop -p Remote \
    >> "$evidence/session.txt"
fi

wpctl status > "$evidence/wpctl.txt"
pactl info > "$evidence/pactl-info.txt"
pactl list short sinks > "$evidence/pactl-sinks.txt"
pactl list short sources > "$evidence/pactl-sources.txt"

cat /proc/asound/cards > "$evidence/alsa-cards.txt"
aplay -l > "$evidence/alsa-playback.txt"
arecord -l > "$evidence/alsa-capture.txt"
lsusb > "$evidence/usb.txt"
bluetoothctl devices > "$evidence/bluetooth.txt"
~~~

If a command is unavailable, record that fact instead of installing unrelated utilities
mid-run. Record desktop scale and monitor layout manually because the environment
variables do not prove per-display scale.

### Artifact and package metadata

~~~bash
appimage="/path/to/Anarlog.AppImage"
deb="/path/to/anarlog.deb"

sha256sum "$appimage" "$deb"
file "$appimage" "$deb"
dpkg-deb --field "$deb" Package Version Architecture Depends
dpkg-deb --contents "$deb"

chmod +x "$appimage"
"$appimage" --appimage-version

sudo apt install "$deb"
pkg="$(dpkg-deb --field "$deb" Package)"
dpkg-query -W "$pkg"
file "$(command -v anarlog)"
ldd "$(command -v anarlog)"
~~~

For the .deb upgrade test:

~~~bash
previous_deb="/path/to/previous.deb"
candidate_deb="/path/to/candidate.deb"

sudo apt install "$previous_deb"
sudo apt install "$candidate_deb"
pkg="$(dpkg-deb --field "$candidate_deb" Package)"
dpkg-query -W "$pkg"
~~~

For removal, use sudo apt remove "$pkg", not purge.

### Application logs and audio backend markers

The stable bundle identifier is com.hyprnote.stable. The tracing plugin writes
app.log and up to five rotated files, app.log.1 through app.log.5, under the
Tauri app log directory.

~~~bash
data_dir="$XDG_DATA_HOME"
if [ -z "$data_dir" ]; then
  data_dir="$HOME/.local/share"
fi

log_dir="$data_dir/com.hyprnote.stable/logs"
find "$log_dir" -maxdepth 1 -type f -name "app.log*" -print
tail -n 500 "$log_dir/app.log"

grep -E "mic_input_initialized|pipewire_capture_initialized|pipewire_capture_unavailable|pulseaudio_capture_initialized|mic_stream_error|queue_overflow|capture_stream_failed" \
  "$log_dir"/app.log*
~~~

Use com.hyprnote.staging for a staging package and com.hyprnote.dev for a dev
build. To capture the foreground process as well as file logs:

~~~bash
RUST_LOG=info,audio_actual=debug ./Anarlog.AppImage 2>&1 |
  tee anarlog-console.log
~~~

The speaker implementation tries PipeWire first and falls back to a PulseAudio monitor
source. Use a disposable launch to exercise the fallback:

~~~bash
PIPEWIRE_REMOTE=anarlog-invalid \
RUST_LOG=info,audio_actual=debug \
./Anarlog.AppImage 2>&1 |
  tee anarlog-pulse-fallback.log
~~~

The run passes L-AUD-07 only if the log contains pipewire_capture_unavailable followed
by pulseaudio_capture_initialized and the saved system-audio stream is non-silent.
If PIPEWIRE_REMOTE does not isolate the PipeWire connection on the tested distro, record
the command as BLOCKED and link the follow-up issue rather than claiming the fallback
passed.

The stable app database normally lives at
$data_dir/anarlog/app.db, but a migrated install can intentionally retain the legacy
hyprnote or bundle-identifier directory.

~~~bash
for db in \
  "$data_dir/anarlog/app.db" \
  "$data_dir/hyprnote/app.db" \
  "$data_dir/com.hyprnote.stable/app.db"
do
  if [ -f "$db" ]; then
    stat "$db"
  fi
done

cache_dir="$XDG_CACHE_HOME"
if [ -z "$cache_dir" ]; then
  cache_dir="$HOME/.cache"
fi

find "$cache_dir/char/cloudsync" -type f -name "cloudsync.so" -exec file {} \;
busctl --user list | grep -F "org.freedesktop.secrets"
~~~

The Secret Service command proves only service availability. Verify Anarlog credential
metadata through the desktop credential manager or Seahorse without exposing values. The
current secure-store service name is com.anarlog.stable.secure-store. The CloudSync
cache path intentionally still uses char/cloudsync.

For a desktop or audio-service failure, collect a bounded journal slice:

~~~bash
journalctl --user --since "-30 min" --no-pager |
  grep -Ei "anarlog|hyprnote|pipewire|wireplumber|pulse|gnome-keyring"

if command -v coredumpctl >/dev/null 2>&1; then
  coredumpctl --since "-30 min" info anarlog
fi
~~~

If Flatpak is later tested, its host-visible log path is expected under:

    $HOME/.var/app/com.hyprnote.Hyprnote/data/com.hyprnote.Hyprnote/logs/

Confirm the path from the running sandbox rather than assuming the native-package path.

### Optional source-build smoke

The repository's current Linux setup and E2E path uses these commands. The setup scripts
are Ubuntu-oriented and are not yet a clean ARM64 setup path; that remains tracked by
ANLG-164. These commands are useful before packaging, but they do not replace tests of
published AppImage and .deb artifacts:

~~~bash
bash scripts/setup-linux-tauri.sh
bash scripts/setup-linux-others.sh
pnpm -F ui build
pnpm -F desktop typecheck
cargo check -p desktop --target x86_64-unknown-linux-gnu
POSTHOG_API_KEY=phc_local_smoke \
VITE_API_URL=https://api.anarlog.so \
pnpm -F desktop tauri build --no-bundle --target x86_64-unknown-linux-gnu --config ./src-tauri/tauri.conf.staging.json --features devtools
~~~

## Community run template

Use this for both x86_64 and ARM64 submissions:

| Field                    | Value                                      |
| ------------------------ | ------------------------------------------ |
| Run ID                   | L-RUN-YYYYMMDD-00                          |
| Tester                   | TBD                                        |
| Physical, community, VM  | TBD                                        |
| Candidate artifact       | TBD                                        |
| Candidate SHA-256        | TBD                                        |
| Architecture             | x86_64 / ARM64                             |
| Distro and version       | TBD                                        |
| Kernel                   | TBD                                        |
| Desktop/display server   | TBD                                        |
| PipeWire/Pulse versions  | TBD                                        |
| OEM/model                | TBD                                        |
| Input/output devices     | TBD                                        |
| Bluetooth/USB details    | TBD                                        |
| Meeting application      | TBD                                        |
| Package type             | AppImage / .deb                            |
| Test IDs covered         | TBD                                        |
| Result                   | PASS / FAIL / BLOCKED                      |
| Redacted evidence link   | TBD                                        |
| Tracking issue           | TBD                                        |

Do not ask a community tester to upload their database, credentials, full transcript, or
unredacted meeting recording.

## Failure record template

Copy this section for each failure:

| Field                  | Value                                 |
| ---------------------- | ------------------------------------- |
| Test ID                | L-XXX-00                              |
| Run ID                 | L-RUN-YYYYMMDD-00                     |
| Candidate hash         | TBD                                   |
| Severity               | Release blocker / high / medium / low |
| Environment cell       | L-ENV-...                             |
| Package ID             | L-PKG-...                             |
| Device and driver      | TBD                                   |
| Meeting application    | TBD                                   |
| Desktop/display server | TBD                                   |
| Started at             | ISO-8601 timestamp with timezone      |
| Expected               | TBD                                   |
| Observed               | TBD                                   |
| Reproduction steps     | TBD                                   |
| Log excerpt or link    | Redacted attachment link              |
| Screenshot/video link  | Redacted attachment link              |
| Tracking issue         | TBD                                   |
| Retest result          | NOT RUN                               |

## Ship and no-ship rules

Mark the candidate BETA SHIP only when all of the following are true:

- Every required test is PASS for the exact published artifact hashes.
- L-ENV-X64-PHYSICAL and L-ENV-ARM64-PHYSICAL each contain a physical or
  trusted-community core audio, credential, CloudSync, and lifecycle run.
- L-ENV-ARM64-VM and L-ENV-X64-CLEAN pass their assigned package and UI smoke.
- The required evidence set collectively covers built-in/default, USB, and Bluetooth
  audio hardware.
- x86_64 AppImage and .deb install, launch, update, and removal paths pass.
- The published ARM64 .deb is correctly labeled, launches natively, opens SQLite, and
  loads the ARM64 CloudSync extension.
- PipeWire and the PulseAudio fallback either work or fail with an actionable visible
  state; neither can silently record empty system audio.
- Secret Service and CloudSync survive restart, lock/unlock, logout/login, and reboot
  without plaintext fallback or lost data.
- Permission copy and feature gating match Linux capabilities.
- Notification, tray, autostart, deep-link, single-instance, and declared meeting
  detection behavior pass on the GNOME/Wayland baseline.
- No open release-blocking issue remains.

Mark the candidate NO SHIP when any of these conditions is present:

- Either architecture lacks the required physical or trusted-community core run.
- An artifact hash or architecture does not match its published checksum or label.
- A clean AppImage or .deb install fails because of an undeclared runtime dependency.
- Microphone or system audio is silent, stalls, cannot recover or fail visibly after
  device loss, or is proven only in a VM.
- Credentials disappear, are written to plaintext, or cannot recover after Secret
  Service becomes available again.
- CloudSync fails to load the correct architecture library or loses, duplicates, or
  crosses account data.
- Upgrade or removal corrupts or unexpectedly deletes user data.
- The UI advertises macOS-only permission flows, models, shortcuts, overlays, or
  integrations that silently no-op on Linux.

The initial Linux beta may defer all of the following if the product and release notes
say so explicitly:

- Flatpak and Flathub distribution.
- Fedora and other non-Ubuntu distros.
- KDE Plasma and X11 support.
- Teams-specific confidence coverage.
- Global shortcuts, dictation, floating controls, or live captions that are fully hidden
  or clearly unavailable on Linux.
- ARM64 AppImage when ARM64 .deb is the declared ARM64 package.

It may not defer both x86_64 and ARM64 real-hardware/community evidence, x86_64
AppImage and .deb packaging, a declared ARM64 package, core microphone and system-audio
capture, durable local data, Secret Service, CloudSync, or the GNOME/Wayland desktop
behaviors advertised to beta users.

## Hardware purchase decision

After the required community request window closes, record:

| Question                                                     | Answer |
| ------------------------------------------------------------ | ------ |
| Is either required architecture cell still missing?          | TBD    |
| Is any built-in/default, USB, or Bluetooth class uncovered?  | TBD    |
| Are failures reproducible only on a specific OEM/driver?      | TBD    |
| Can a trusted tester provide logs and a reliable retest loop? | TBD    |
| Would purchased hardware close a release-blocking cell?       | TBD    |
| Decision and rationale                                       | TBD    |

Purchase dedicated Linux hardware when it closes a release-blocking physical cell or
provides a repeatable reproduction path that trusted community testing cannot provide.
Do not buy hardware based on architecture alone.

## Run ledger

Add one row per completed run. Keep detailed evidence in the linked attachment rather
than committing it to the repository.

| Run ID | Date | Candidate SHA-256 | Environment cell | Package ID | Scope | Result  | Evidence | Tester |
| ------ | ---- | ----------------- | ---------------- | ---------- | ----- | ------- | -------- | ------ |
| TBD    | TBD  | TBD               | TBD              | TBD        | TBD   | NOT RUN | TBD      | TBD    |

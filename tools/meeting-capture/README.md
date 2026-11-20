# Anarlog meeting capture helper

`anarlog-meeting-capture` records a meeting's microphone and call audio on one host clock for
Anarlog's native meeting engine (`crates/audio-actual/src/native_meeting`). Where the helper is
installed, the desktop app records every two-track capture of the Mac's current default input with
it. A named microphone, onboarding's system-audio capture, other platforms, and builds without the
helper keep the existing engine. Release builds don't bundle the helper yet. Qualification builds
use `apps/desktop/src-tauri/scripts/prepare-meeting-helpers.mjs`; local trials can select a freshly
built executable with `ANARLOG_MEETING_CAPTURE_HELPER`. Debug builds do not discover `.build`
artifacts automatically, because an old helper there could silently change the recording engine.

- The call track is a Core Audio process tap of the confirmed call's app and its helper processes,
  or of the whole Mac except the helper when no single call is confirmed. A re-opened capture taps
  exactly the app it confirmed and never broadens to the whole Mac.
- The microphone is the Mac's default input. Voice processing runs on speakers and not on
  headphones; the mode is reported so Anarlog never cancels echo a second time. The helper never
  changes the default input and never prompts for permission.
- Both tracks are 48 kHz mono float CAF files in Anarlog's staging directory, placed by host time.
  Device clock drift stays within tolerance, converters are primed and drained, paused audio is
  removed before resampling, and pauses, late starts and device gaps are silence. Device changes
  rebuild the tap or microphone instead of ending the recording. The microphone follows a new
  default input even when AVAudioEngine doesn't report it, and is rebuilt when its buffers stop
  arriving; silent buffers still count as arriving. A plain microphone stays where AVAudioEngine
  binds it, often a private aggregate of the default input and output, and is rebuilt as soon as
  the engine moves it to another input or rate, as an output change can.
- Accessibility reads Zoom, Meet in Chromium browsers, Teams and Webex windows for one confirmed
  call and visible roster rows. No app has shown a speaking signal, so observations never carry one.

## Build and test

Requires macOS 15+ and Xcode command-line tools. Tests use generated PCM and fake Core Audio
backends; set `CI=1` to skip the few that touch the audio engine of a real Mac.

```sh
swift build -c release --package-path tools/meeting-capture --product anarlog-meeting-capture
CI=1 swift test --package-path tools/meeting-capture
```

## Protocol

```
anarlog-meeting-capture version | calls
anarlog-meeting-capture record --output DIR --capture-id ID
    [--scope discover | --scope system | --scope call --pid PID --bundle-id ID --platform P [--call-url URL]]
    [--microphone NAME_OR_UID] [--microphone-processing automatic|voice|voice-advanced-ducking|plain]
    [--accessibility on|off] [--seconds N]
```

`DIR` must exist, be private (0700) and empty. stdin takes `pause`, `resume` and `stop`; EOF and
SIGINT/SIGTERM also stop. `automatic` is the default microphone processing; the other modes are for
qualification. Voice processing ducks every other app's audio, the call included: on built-in
speakers the call measured 8 dB quieter with `automatic` and `voice-advanced-ducking`, both at
Apple's lowest level (Minimum), and macOS has no public setting that turns ducking off. `plain`
records the raw microphone with no ducking, and the adapter cancels the call's echo with Anarlog's
echo canceller against the call track; it is the speaker candidate under device qualification.
Default production cutover remains pending that choice, signed app permission checks, and
Bluetooth/USB/input-switch qualification. Keep the existing audio backend.

stdout is framed: `kind: u8`, `length: u32 LE`, `payload`. Kind `E` is one event line,
`event=<name> key=value ...`, with percent-encoded values. Kind `P` is PCM: `track: u8`
(0 microphone, 1 remote), `frame: u64 LE` on the shared 48 kHz timeline, then f32 LE samples.
`DIR/capture.journal` holds the event lines, synced per line, beside `microphone.caf` and
`remote.caf`.

| Event | Meaning |
| --- | --- |
| `prepared` | journal opened at frame 0 (`captureId`, `originHostSeconds`) |
| `started` | running; `scope` `application` or `system`, `pid`, `bundleId`, `platform`, `callUrl`, `microphoneName`, `micProcessing` |
| `microphone-configured`, `-interrupted`, `-recovered`, `-unavailable`, `-startup-retry` | microphone graph and device changes; `voiceProcessing` is provenance; `microphone-configured` names the default input the graph records (`microphoneId`, `microphoneName`), never the engine's aggregate; `microphone-interrupted` has a `reason`: `configuration-changed`, `input-changed`, `output-changed`, `audio-service-restarted` or `stalled` (no buffers arrived) |
| `microphone-voice-processing` | read back from the running voice processing unit: `duckingLevel` (0 default, 10 minimum, 20 medium, 30 maximum), `advancedDucking`, `agc`; `message` if unreadable |
| `call-audio-configured`, `-interrupted`, `-unavailable`, `-processes-changed` | process tap lifecycle |
| `*-buffer-dropped` | an unusable device buffer stayed silence |
| `tick frame` | live watermark, 0.5 s behind the host clock; advances through pauses (stdout only) |
| `paused frame`, `resumed frame`, `command-error` | acknowledgements of stdin commands |
| `meeting-observation`, `meeting-observation-unavailable` | every accepted read refreshes live roster evidence; the journal keeps changes only; stamped with the read's start `frame` |
| `stopping`, `ended` | `ended` has `reason`, `frames` (both tracks padded to it) and per-track frame counts and peaks |
| `live-output-overrun` | Anarlog stopped reading; the capture fails, its tracks are kept |
| `failed code message` | fatal; exit status 1. Codes include `microphone_not_default`, `microphone_permission`, `call_app_unavailable`, `staging_invalid`, `live_output_overrun` |

`calls` prints one `call` event per supported app (`bundleId`, `platform`, `status`, `callUrl`),
then `done`. `version` prints `protocol=1`.

## Provenance

Ported from Char (Fastrepl, Inc.) commit `d4608723d8ac328ab91091803a08c4585a0dc244`,
`tools/meeting-capture-lab`:

- Copied with renames only: `Sources/AudioEngineGuard`, `Sources/MeetingAccessibility`,
  `CaptureCore/{AudioTrack,CallAudio,OutputRoute,Timeline,VoiceMicrophone,MeetingObservation}.swift`,
  and their tests under `Tests/`.
- Modified: `CaptureCore/NativeCapture.swift` (Anarlog-owned staging, line-protocol journal, live
  output, watermark, coded refusals, no permission prompt) and `CaptureCLI/main.swift` (rewritten
  for this protocol). New: `CaptureCore/Wire.swift` and `LiveOutputTests.swift`.
- Not ported: Char's speech, diarization (FluidAudio), playback mixdown, and manual speaker cues.

Char's upstream source is distributed under GPL-3.0-or-later. On 2026-11-19, Fastrepl authorized distribution of its Char code ported here under Anarlog's MIT license.

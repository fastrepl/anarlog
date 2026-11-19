# Anarlog Camera

A camera named **Anarlog Camera** that shows the person's usual camera, with an
"{anarlog} is taking notes" badge while Anarlog records a meeting, so others in the call see that
Anarlog is taking notes. It is opt-in, off by default, macOS only, and exists only in direct
distribution builds packaged with it. The App Store build never includes it. Ported from Char's
camera watermark; see [PROVENANCE.md](PROVENANCE.md).

Every identity is Anarlog's own, so it installs and runs beside Char Camera: helper
`so.anarlog.camera`, extension `so.anarlog.camera.extension`, App Group `<TEAM>.so.anarlog.camera`,
login agent `so.anarlog.camera.agent`, state folder
`~/Library/Application Support/so.anarlog.camera`, and its own device and stream UUIDs. The helper
never wraps a virtual camera, so the two watermark cameras can't feed each other.

## How it works

- `WatermarkExtension`: a CoreMediaIO Camera Extension that macOS runs as its own user. It publishes
  one device with a **source** stream that meeting apps read and a **sink** stream the helper writes.
  Each sink frame goes out as it arrives; while none has arrived for 0.5 s, it sends an idle frame
  (dark, the Anarlog wordmark, "Open Anarlog to use this camera") at 30 fps. Only
  `Anarlog Camera.app`, signed by the same team as the extension, may write, one process at a time
  (`SinkGate`). An extension without a team signature (an ad-hoc dev build) admits nobody. It
  publishes the signing IDs of the apps streaming from it as the custom property
  `4cc_cnsm_glob_0000`, read from the kernel with `csops` (`SignedClient`).
- `WatermarkHelper` (`anarlog-camera`, inside `Anarlog Camera.app`):
  - `agent`: the login agent (`Contents/Library/LaunchAgents/so.anarlog.camera.agent.plist`, KeepAlive)
    that `serve` registers with `SMAppService`. Twice a second it reads `state.json` (camera on,
    recording plus Anarlog's pid, style, background, `background_id`, `camera_request`) and who uses
    the camera. Whenever an app does and the camera is on, it opens the real camera (the one macOS
    prefers, else the built-in one; never a virtual camera), scales to 1280x720, puts the background
    behind the person, draws the badge while Anarlog records and is alive, and enqueues to the sink.
    When the last app leaves, it lets the camera go. It writes `status.json` on change and at least
    every 2 s.
  - `serve`: Anarlog runs it briefly and talks JSON lines on stdin/stdout (`activate`,
    `deactivate`, `enable_agent`, `disable_agent`, `check`; events `ready`, `extension`, `agent`,
    `camera`, `error`). It never opens the camera. It exits when stdin closes.
  - `render`: one frame through the live compositor without the extension, over `--input` or a grey
    frame. `render --camera` reads the real camera for timing; it is a developer tool only.
- `WatermarkKit`: the badge styles (`WatermarkStyle`, `WatermarkLayout`, `Watermark`), the idle frame,
  the compositor, backgrounds, the Anarlog wordmark, `FeedState`, the protocol and the agent files.

## Output receipts

An app's own blur or background cuts out everything but the person, the badge included, so Anarlog
Camera can hide the room itself. Anarlog turns a meeting app's effect off only on a receipt that
frames hiding the room for the exact current request went out (`ProtectedOutput`):

1. Each `setBackground` gets a new `background_id` in `state.json`.
2. Each frame the helper enqueues gets a sequence number (`kCMIOSampleBufferAttachmentKey_SequenceNumber`)
   and the `background_id` it was drawn for.
3. The extension reports a frame's number only after sending it to meeting apps
   (`notifyScheduledOutputChanged`), and advertises that with `4cc_orcp_glob_0000 = "sent"`.
4. The agent writes `protected_background_id` only while the latest frame out, within 1 s, hid the
   room for the current request.
5. Anarlog accepts that only from a status written after it began waiting, for the request it last
   wrote, then re-checks intent before touching the meeting app.

Frames a meeting app already buffered are beyond what Anarlog can see.

## Backgrounds

`background` is one JSON value, `{"kind","image","custom"}`: `none` (default), `blur`, or `image`
with `studio`, `dusk` or `daylight` (drawn from code in `BackgroundScene`, nothing licensed) or
`custom` (a JPEG/PNG data URL). A bad value draws Studio, never the room the person asked to hide.
`PersonSegmenter` uses Vision's person segmentation at `balanced` every other frame, smoothed between
frames. Char measured this at 7.5-13.7 ms a frame (p95 14-22 ms) and 16-17% of one core at 1280x720
on an M5 Max. Anarlog has not re-measured it.

## Build and check

```sh
swift test --package-path tools/camera-watermark
node tools/camera-watermark/scripts/bundle.mjs            # ad-hoc dev bundle in bundle.noindex/
tools/camera-watermark/.build/release/anarlog-camera render --output /tmp/frame.png \
  [--input photo.jpg] [--background '{"kind":"blur"}'] [--tile 640x360] [--time 1.2] [--idle]
```

## Signing and activation requirements

macOS activates the extension only when all of these hold. None of them are in this repository:

1. The helper has `com.apple.developer.system-extension.install`, granted only by a provisioning
   profile for App ID `so.anarlog.camera` (System Extension capability). Pass `--app-profile`.
   Without it, activation fails with `missing_entitlement`.
2. The extension's profile for `so.anarlog.camera.extension` covers App Group
   `<TEAM>.so.anarlog.camera`. Pass `--extension-profile` and `--team-id`.
3. Both profiles name the Developer ID Application certificate passed as `--identity`.
4. The containing app sits under `/Applications`, at any depth:
   `Anarlog.app/Contents/Helpers/Anarlog Camera.app`. Elsewhere sysextd answers `not_in_applications`.
5. The extension is notarized. Notarizing Anarlog.app covers the nested copy.
6. Packaging passes Anarlog's version as `--version short/build`, because sysextd replaces a
   registered extension only when the version changes.

The person then allows it in System Settings → General → Login Items & Extensions → Camera
Extensions, and grants Anarlog Camera camera access. Visible remote video still needs live
acceptance in each meeting app.

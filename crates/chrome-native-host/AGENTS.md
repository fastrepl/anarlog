# chrome-native-host

Chrome native-messaging host for the Meet extension (`browser-meeting-extension/`). Chrome starts it per extension port; it proxies that port to the running Anarlog app and holds no recording state. The app side, including the state machine, is `crates/transcription-core/src/browser_meeting`.

```
Meet tab (activeTab, after a toolbar click)
  │ chrome.runtime.connectNative("so.anarlog.browser_meeting")
  ▼
this host ── stdin/stdout, Chrome framing (u32 native-endian length + JSON)
  │ hello { secret, origin, browser_pid } on the app's 0600 Unix socket
  ▼
browser_meeting::serve_connection → Relay → BrowserMeetingRoute → BrowserMeetingPort
  (the one meeting runtime and recorder path)
```

## Rules

- Find the app through endpoint records in `{dirs::data_dir()}/anarlog-browser-meeting/*.json` (0600, written by `browser_meeting::serve`). Use only records whose `allowed_origins` contains the `chrome-extension://<id>/` origin Chrome passed, newest first; skip records another user could write.
- Present the record's per-launch secret, the origin, and the parent process ID (the browser) in the first frame. Forward nothing before the app's `welcome`, then tell the extension `{"event":"connected"}`. An old host never sends `connected`, so the extension times out instead of waiting.
- Forward only the eight `meeting_*` messages from Chrome. Pass the app's events back unchanged. When Chrome closes stdin, shut down the socket's write half; the app then detaches the tab. A recording the tab started stops, and one it joined keeps going.
- Keep it std-only: no async runtime, and no dependency on `transcription-core`. Both sides test against `crates/transcription-core/src/browser_meeting/fixtures/wire.json`.
- Windows is not supported yet. The host tells the extension so.

## Registration (integration)

The binary is bundled as `binaries/char-chrome-native-host` (see `crates/xtask/src/prepare_binaries.rs`). The app writes the host manifest `so.anarlog.browser_meeting.json` into each installed Chromium browser's `NativeMessagingHosts` directory. That manifest's `allowed_origins` lists the same extension origins that the app passes to `browser_meeting::serve`.

```
cargo test --locked -p chrome-native-host
```

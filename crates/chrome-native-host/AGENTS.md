# chrome-native-host

Rust binary that bridges the Chrome extension to the Char desktop app.
It is registered as a Chrome Native Messaging host, so Chrome spawns it as a subprocess and communicates over stdio.

## System diagram

```
Google Meet tab
  │  chrome.runtime.sendMessage(payload)
  ▼
background.js (service worker)
  │  port.postMessage(payload)   [Chrome Native Messaging]
  ▼
char-chrome-native-host  ← this binary
  │  writes atomically
  ▼
chrome_state.json  (on disk)
  │  polled every 500 ms
  ▼
GoogleMeetWatcher (crates/detect)
  │  fires DetectEvent
  ▼
Tauri plugin → React/Zustand
```

## Native Messaging wire protocol

Chrome Native Messaging uses **stdin/stdout** with a simple framing:

```
┌─────────────────┬──────────────────────────────┐
│  4 bytes LE u32 │  N bytes UTF-8 JSON           │
│  (message len)  │  (message body)               │
└─────────────────┴──────────────────────────────┘
```

- Length is **little-endian unsigned 32-bit** integer.
- Body is a UTF-8 JSON object.
- The binary reads messages in a loop until EOF (Chrome closed the port).

## Incoming message contract

Two message types are sent by `background.js`:

### `meeting_state` — meeting is active

```json
{
  "type": "meeting_state",
  "url": "https://meet.google.com/abc-defg-hij",
  "is_active": true,
  "muted": false,
  "participants": [
    { "name": "Alice", "is_self": true },
    { "name": "Bob",   "is_self": false }
  ]
}
```

### `meeting_ended` — tab closed / navigated away

```json
{
  "type": "meeting_ended",
  "url": "https://meet.google.com/abc-defg-hij",
  "is_active": false
}
```

Any unknown `type` is silently ignored.

## Output: `chrome_state.json`

Written to `{data_dir}/char/chrome_state.json` after every message.
`data_dir` is platform-specific (`dirs::data_dir()`):
- macOS: `~/Library/Application Support`
- Linux: `~/.local/share`
- Windows: `C:\Users\{user}\AppData\Roaming`

**Schema:**

```json
{
  "version": 1,
  "timestamp_ms": 1700000000000,
  "meeting": {
    "url": "https://meet.google.com/abc-defg-hij",
    "is_active": true,
    "muted": false,
    "participants": [
      { "name": "Alice", "is_self": true }
    ]
  }
}
```

`meeting` is `null` when the meeting has ended or `is_active` is false.

**Write is atomic** — a `NamedTempFile` in the same directory is written and then renamed,
so the watcher never reads a partial file.

`timestamp_ms` is set to the current Unix epoch milliseconds on every write.
The watcher in `crates/detect` treats state older than 30 seconds as stale.

## Testable units

| Function | What it does | How to test |
|---|---|---|
| `read_message` | Decodes framed bytes from a `Read` | Feed a `Cursor<Vec<u8>>` |
| `process_message` | Maps `IncomingMessage` → `Option<MeetingState>` | Pure function, no I/O |
| `write_state` | Atomically writes JSON to a `Path` | Pass a `tempdir` path |
| `run` | Full loop: read → process → write | Pipe encoded messages through a `Cursor`, assert file contents |

## Bundle configuration

For production, the binary is included in the app bundle as a Tauri `externalBin`
(`tauri.conf.json`), placing it at `Contents/MacOS/char-chrome-native-host` next
to the main executable.

**How `prepare-binaries.mjs` works (runs as `beforeBundleCommand`):**

1. Reads `TAURI_ENV_TARGET_TRIPLE` — the env var Tauri sets to the actual build
   target, not the host. This matters for cross-compilation: the CI macOS job
   builds both `aarch64-apple-darwin` and `x86_64-apple-darwin` from a single
   ARM runner, so host ≠ target on the x86_64 build.

2. Runs `cargo build --release --target $triple -p chrome-native-host` with
   `cwd` set to `apps/desktop/src-tauri/` so that `.cargo/config.toml`'s
   `target-dir = "target"` applies. Binary lands at
   `apps/desktop/src-tauri/target/$triple/release/char-chrome-native-host[.exe]`.

3. Copies it to `binaries/char-chrome-native-host-$triple[.exe]`, which is what
   `externalBin: ["binaries/char-chrome-native-host"]` expects.

No CI steps or `build.rs` changes are needed — `beforeBundleCommand` runs inside
`tauri build` after the main Cargo step, so the cross-compilation toolchain is
already present.

For development (`debug_assertions`), no bundling is needed — the runtime resolves
directly to `target/debug/char-chrome-native-host` built by Cargo as part of the workspace.

## Running tests

```
cargo test --package chrome-native-host
```

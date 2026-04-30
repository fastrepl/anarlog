# Spec: `crates/soniqo-bridge` — Port Soniqo on-device transcription into anarlog

## Mission

Port the **Soniqo `speech-swift` Swift package integration** from `fastrepl/unsigned-char-old` into a new isolated Rust crate inside this monorepo. **Do NOT touch any adapters or wire this into `owhisper-client` yet** — that's a separate task.

Outcome: a new crate `crates/soniqo-bridge` that:
1. Compiles (`cargo check -p soniqo-bridge` succeeds on macOS)
2. Builds the Soniqo Swift package via `swift-rs` build script
3. Exposes a clean public Rust API mirroring `unsigned-char-old/src-tauri/src/asr.rs`
4. Has a passing smoke unit test (or at minimum a doctest that compiles)

## Source material — reference implementations

**The reference files have already been staged locally for you at:**

```
.tmp/reference/
├── Cargo.toml                                 # original src-tauri/Cargo.toml from unsigned-char-old
├── build.rs                                   # original build.rs (swift-rs SwiftLinker invocation)
├── src/
│   ├── asr.rs                                 # 555 lines — Rust-side swift_rs declarations + TranscriptionManager
│   ├── speech_models.rs                       # 462 lines — model registry + cache management
│   └── audio_capture.rs                       # reference for LiveCaptureSession (DO NOT port — out of scope)
└── swift-permissions/
    ├── Package.swift                          # SwiftPM manifest with soniqo deps
    ├── Package.resolved                       # pinned versions
    └── src/
        ├── speech_bridge.swift                # 1847 lines — the main Swift bridge
        └── lib.swift                          # other Swift bridges (audio permissions, etc.)
```

**Read these files via plain filesystem (`read_file` / `cat`)** — do NOT use github_fetch MCP, it's been failing. All needed source is local.

The original repo is `fastrepl/unsigned-char-old`. **DO NOT copy verbatim — adapt to the monorepo crate shape described below.**

## Target crate layout

```
crates/soniqo-bridge/
├── Cargo.toml
├── build.rs              # invokes swift-rs SwiftLinker
├── README.md             # 1-paragraph: what this crate is, how to update soniqo version
├── swift/
│   ├── Package.swift     # SwiftPM manifest with soniqo/speech-swift dep
│   ├── Package.resolved  # pin the resolved version
│   └── Sources/
│       └── SoniqoBridge/
│           └── speech_bridge.swift   # ported, scoped to speech only
└── src/
    ├── lib.rs            # public surface, swift! extern declarations
    ├── error.rs          # Error/Result types
    ├── models.rs         # SpeechModelKind enum + registry (port speech_models.rs)
    ├── transcribe.rs     # batch transcribe API
    └── live.rs           # LiveTranscriptionState + start/append/stop API (the streaming surface)
```

## Workspace integration

1. Add `crates/soniqo-bridge` as a member of the root `Cargo.toml` workspace.
2. Add it to the workspace dependency catalog (look at how other crates are listed).
3. **Do NOT add it as a dep of any other crate yet.** Standalone only this round.

## Public API contract (Rust)

The crate must expose at minimum:

```rust
// src/lib.rs

pub use error::{Error, Result};
pub use models::{SpeechModelKind, ModelDownloadState};
pub use transcribe::transcribe_audio_file;
pub use live::{LiveTranscriptionState, LiveTranscriptEntry, ProcessingMode};

pub fn model_cache_dir(model_id: &str) -> Result<PathBuf>;
pub fn model_download_state(model_id: &str) -> Result<ModelDownloadState>;
pub fn start_model_download(model_id: &str) -> Result<()>;
pub fn reset_model(model_id: &str) -> Result<()>;
```

Live transcription:
```rust
pub fn start_live_transcription(
    mode: ProcessingMode,         // realtime | batch
    model_id: &str,
    recording_path: &Path,
    language: &str,
) -> Result<LiveTranscriptionState>;

pub fn append_live_samples(
    mixed_samples: &[u8],
    microphone_samples: &[u8],
    system_samples: &[u8],
) -> Result<LiveTranscriptionState>;

pub fn live_transcription_state() -> Result<LiveTranscriptionState>;
pub fn request_live_stop() -> Result<LiveTranscriptionState>;
pub fn stop_live_transcription() -> Result<LiveTranscriptionState>;
```

(Names align with the Swift `_speech_live_transcription_*` functions in the reference repo.)

Diarization & speaker embedding (port if present in reference):
```rust
pub fn diarize_audio_file(audio_path: &Path, speaker_count: i64) -> Result<DiarizationResult>;
pub fn embed_speaker_audio_file(audio_path: &Path, speakers: &[Speaker]) -> Result<Vec<SpeakerEmbedding>>;
```

## Hard constraints

1. **macOS-only.** Gate everything with `#[cfg(target_os = "macos")]`. On non-macOS, the crate must still compile (provide stubs that return `Error::UnsupportedPlatform`).

2. **`swift-rs` version: 1.0.7** — match the reference repo exactly.

3. **Soniqo speech-swift version: 0.0.9** — match the reference's `Package.resolved`. Don't bump.

4. **Minimum macOS deployment target: 15.0** — same as reference.

5. **Do not pull in `LiveCaptureSession` from `audio_capture.rs`.** That's a higher-level abstraction tied to the reference repo's audio system. The bridge crate only deals with raw byte buffers. Audio capture wiring happens at the consumer layer (later task).

6. **Do not modify `crates/owhisper-client/`.** Don't touch any adapter. Don't touch `AdapterKind`. Don't touch `cactus`. **This task is purely additive.**

7. **No unsafe code outside what `swift-rs` requires.** Wrap the FFI in safe Rust APIs.

8. **Comments only when non-obvious** (per repo `AGENTS.md`). Explain "why", not "what".

## Build verification

Codex must verify all of these pass before declaring the task done:

```bash
# 1. Workspace recognizes the new crate
cargo metadata --format-version 1 --no-deps | jq '.packages[] | select(.name == "soniqo-bridge") | .name'

# 2. Compiles
cargo check -p soniqo-bridge

# 3. Builds (this actually runs swift-rs build script)
cargo build -p soniqo-bridge

# 4. Tests
cargo test -p soniqo-bridge --lib

# 5. Format
pnpm exec dprint fmt 2>/dev/null || rustfmt --edition 2024 crates/soniqo-bridge/src/*.rs crates/soniqo-bridge/build.rs
```

If `cargo build` fails because soniqo's Swift package fails to fetch on the build agent (network/auth issues), document the failure in `crates/soniqo-bridge/README.md` "Known build issues" section and proceed — `cargo check` is the must-pass gate.

## Branch + commit discipline

You are on the GitButler branch `soniqo-bridge` (workspace branch is `gitbutler/workspace`). Use **`but` for all commits**, NOT bare `git commit`:

```bash
# After making related changes, commit them as a group
but commit -m "feat(soniqo-bridge): add crate skeleton"
but commit -m "feat(soniqo-bridge): port speech_bridge.swift"
but commit -m "feat(soniqo-bridge): port asr.rs to lib.rs/transcribe.rs/live.rs"
but commit -m "feat(soniqo-bridge): port speech_models.rs"
but commit -m "feat(soniqo-bridge): wire build.rs + swift package, verify cargo check"
```

Small, intent-titled commits. After each meaningful step, run `but commit`.

When the task is fully complete and verified:
```bash
but push
```

This pushes the `soniqo-bridge` branch to origin. Stop there. **Do NOT open a PR** — John reviews the branch first.

## Out of scope (DO NOT DO)

- ❌ Wiring soniqo-bridge into `crates/owhisper-client/`
- ❌ Adding `AdapterKind::Soniqo` (the local one) — note the Soniox cloud adapter already exists, do not confuse them
- ❌ Removing the cactus adapter or any cactus references
- ❌ Modifying `apps/desktop/` or `plugins/local-stt`
- ❌ Touching CI workflows
- ❌ Updating user-facing docs at `apps/web/`

These are all separate tasks for separate PRs.

## When you're stuck

If swift-rs build issues block you, do this in order:
1. Verify Xcode CLI tools: `xcode-select -p`
2. Verify swift toolchain: `swift --version` (need ≥5.9)
3. Check the reference repo's `build.rs` — replicate it exactly
4. If the soniqo Swift package can't resolve, try `cd crates/soniqo-bridge/swift && swift package resolve` standalone
5. If still stuck: commit what works, write findings to `crates/soniqo-bridge/README.md` "Build status", and exit cleanly

Don't burn cycles trying every variant. Document and exit > spinning.

## Final checklist before declaring done

- [ ] `crates/soniqo-bridge/` exists with all listed files
- [ ] Root `Cargo.toml` workspace updated
- [ ] `cargo check -p soniqo-bridge` passes (must-have)
- [ ] `cargo test -p soniqo-bridge --lib` passes (smoke test exists)
- [ ] No changes to `crates/owhisper-client/`, `apps/`, `plugins/`, or any CI yaml
- [ ] All commits made via `but commit` with intent-titled messages
- [ ] `but push` completed, branch `soniqo-bridge` exists on origin
- [ ] `crates/soniqo-bridge/README.md` written (~10 lines explaining the crate)

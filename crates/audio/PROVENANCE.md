# Meeting audio timeline

`CaptureTimeline::append` and `CaptureTimeline::capture_ms` in `src/meeting.rs` are Rust adaptations of Char's `AudioTimeline.append` and `AudioTimeline.milliseconds`. The port retains Char's rules for merging adjacent spans, rejecting backwards audio, handling timestamps within one millisecond of the last frame, and mapping a word's start and end across a pause. The Rust implementation returns fractional milliseconds.

The upstream repository is `fastrepl/char` at `d4608723d8ac328ab91091803a08c4585a0dc244`. Char's copyright notice names Fastrepl, Inc. (2023–present) and specifies GPL-3.0-or-later. [Upstream source](https://github.com/fastrepl/char/blob/d4608723d8ac328ab91091803a08c4585a0dc244/char/apps/desktop/src/main/native-meeting/audio-timeline.ts) · [Upstream license](https://github.com/fastrepl/char/blob/d4608723d8ac328ab91091803a08c4585a0dc244/LICENSE)

On 2026-11-19, Fastrepl authorized distribution of its Char code ported here under Anarlog's MIT license. [Anarlog licensing policy](../../LICENSING.md#third-party-provenance)

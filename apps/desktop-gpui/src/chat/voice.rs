//! The composer's voice input: shipping's `useDictation` phases and `VoiceStatus` row
//! (`apps/desktop/src/chat/components/input`). Recording and transcription come from a
//! [`VoiceInput`] the dictation service installs; without one, starting fails the way shipping
//! does when the microphone is unavailable.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use desktop_runtime::Result;
use futures::future::BoxFuture;
use gpui::Global;

/// Shipping stops a dictation after five minutes.
pub const MAX_RECORDING: Duration = Duration::from_secs(5 * 60);

/// Shipping's `WAVEFORM_HEIGHTS`, in pixels.
pub const WAVEFORM: [f32; 14] = [3., 7., 5., 10., 6., 12., 8., 4., 9., 6., 11., 5., 8., 3.];

pub trait VoiceInput: Send + Sync {
    /// Starts a recording; the future resolves once the microphone records. Dropping the
    /// recording before it stops, even while it starts, cancels it and deletes its audio.
    fn start(&self) -> (Box<dyn VoiceRecording>, BoxFuture<'static, Result<()>>);
}

pub trait VoiceRecording {
    /// Stops recording and returns the transcript.
    fn stop(self: Box<Self>) -> BoxFuture<'static, Result<String>>;
}

#[derive(Clone)]
pub struct Voice(pub Arc<dyn VoiceInput>);

impl Global for Voice {}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum VoicePhase {
    #[default]
    Idle,
    Starting,
    Recording(Instant),
    Transcribing,
}

impl VoicePhase {
    /// Shipping's `formatElapsedTime`: `m:ss`.
    pub fn elapsed(started: Instant) -> String {
        let seconds = started.elapsed().min(MAX_RECORDING).as_secs();
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tone {
    Warning,
    Error,
}

impl Tone {
    /// Shipping's `TOAST_DURATIONS`.
    pub fn duration(self) -> Duration {
        match self {
            Self::Warning => Duration::from_secs(6),
            Self::Error => Duration::from_secs(5),
        }
    }
}

/// Bar `index`'s height at `delta` through shipping's 700ms `chat-input-waveform` cycle: each
/// bar starts 70ms earlier and swings between 45% and 100% of its height.
pub fn bar_height(index: usize, delta: f32) -> f32 {
    let phase = (delta + index as f32 * 0.1).fract();
    let swing = (1. - (phase * std::f32::consts::TAU).cos()) / 2.;
    WAVEFORM[index % WAVEFORM.len()] * (0.45 + 0.55 * swing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_time_reads_like_shipping_and_stops_at_five_minutes() {
        let now = Instant::now();
        assert_eq!(VoicePhase::elapsed(now), "0:00");
        let started = now - Duration::from_secs(65);
        assert_eq!(VoicePhase::elapsed(started), "1:05");
        let started = now - Duration::from_secs(400);
        assert_eq!(VoicePhase::elapsed(started), "5:00");
    }

    #[test]
    fn waveform_bars_swing_between_45_and_100_percent() {
        assert_eq!(bar_height(0, 0.), 3. * 0.45);
        assert!((bar_height(5, 0.) - 12.).abs() < 0.01);
        assert!((0..14).all(|index| bar_height(index, 0.3) <= WAVEFORM[index]));
    }
}

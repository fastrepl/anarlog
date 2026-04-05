use std::pin::pin;
use std::time::{Duration, Instant};

use hypr_audio::{AudioProvider, CaptureConfig};
use tokio::signal;
use tokio_stream::StreamExt;

use super::AudioMode;
use crate::error::{CliError, CliResult};

const UI_TICK: Duration = Duration::from_millis(250);
const EVENT_TICK: Duration = Duration::from_secs(1);

pub(crate) struct ProgressUpdate {
    pub(crate) elapsed: Duration,
    pub(crate) sample_count: u64,
    pub(crate) audio_secs: f64,
    pub(crate) left_level: f32,
    pub(crate) right_level: f32,
    pub(crate) render_ui: bool,
    pub(crate) emit_event: bool,
}

pub(crate) struct CaptureResult {
    pub(crate) samples: Vec<i16>,
    pub(crate) elapsed: Duration,
    pub(crate) audio_secs: f64,
    pub(crate) stop_reason: StopReason,
}

#[derive(Clone, Copy)]
pub(crate) enum StopReason {
    CtrlC,
    Eof,
}

impl StopReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::CtrlC => "ctrl_c",
            Self::Eof => "eof",
        }
    }
}

pub(crate) async fn capture<A: AudioProvider>(
    audio: &A,
    mode: AudioMode,
    sample_rate: u32,
    chunk_size: usize,
    mut on_progress: impl FnMut(ProgressUpdate) -> CliResult<()>,
) -> CliResult<CaptureResult> {
    let stream = match mode {
        AudioMode::Input => audio
            .open_mic_capture(None, sample_rate, chunk_size)
            .map_err(|e| CliError::operation_failed("open mic capture", e.to_string()))?,
        AudioMode::Output => audio
            .open_speaker_capture(sample_rate, chunk_size)
            .map_err(|e| CliError::operation_failed("open speaker capture", e.to_string()))?,
        AudioMode::Dual => audio
            .open_capture(CaptureConfig {
                sample_rate,
                chunk_size,
                mic_device: None,
                enable_aec: false,
            })
            .map_err(|e| CliError::operation_failed("open dual capture", e.to_string()))?,
    };
    let mut stream = pin!(stream);
    let mut samples = Vec::new();
    let started = Instant::now();
    let mut last_ui = Instant::now() - UI_TICK;
    let mut last_event = Instant::now() - EVENT_TICK;
    let mut stop_reason = StopReason::Eof;

    loop {
        tokio::select! {
            frame = stream.next() => {
                let Some(result) = frame else { break };
                let frame = result
                    .map_err(|e| CliError::operation_failed("audio capture", e.to_string()))?;

                let (left, right) = match mode {
                    AudioMode::Input => {
                        let raw = frame.preferred_mic();
                        samples.extend(raw.iter().map(|&s| to_i16(s)));
                        (peak_level(&raw), 0.0)
                    }
                    AudioMode::Output => {
                        let raw = &frame.raw_speaker;
                        samples.extend(raw.iter().map(|&s| to_i16(s)));
                        (peak_level(raw), 0.0)
                    }
                    AudioMode::Dual => {
                        let (mic, speaker) = frame.raw_dual();
                        for (&m, &s) in mic.iter().zip(speaker.iter()) {
                            samples.push(to_i16(m));
                            samples.push(to_i16(s));
                        }
                        (peak_level(&mic), peak_level(&speaker))
                    }
                };

                let elapsed = started.elapsed();
                let audio_secs = samples_to_audio_secs(samples.len() as u64, sample_rate, mode);
                let render_ui = last_ui.elapsed() >= UI_TICK;
                let emit_event = last_event.elapsed() >= EVENT_TICK;
                if render_ui || emit_event {
                    on_progress(ProgressUpdate {
                        elapsed,
                        sample_count: audio_frame_count(samples.len() as u64, mode),
                        audio_secs,
                        left_level: left,
                        right_level: right,
                        render_ui,
                        emit_event,
                    })?;
                    let now = Instant::now();
                    if render_ui {
                        last_ui = now;
                    }
                    if emit_event {
                        last_event = now;
                    }
                }
            }
            _ = signal::ctrl_c() => {
                stop_reason = StopReason::CtrlC;
                break;
            }
        }
    }

    let elapsed = started.elapsed();
    let audio_secs = samples_to_audio_secs(samples.len() as u64, sample_rate, mode);
    on_progress(ProgressUpdate {
        elapsed,
        sample_count: audio_frame_count(samples.len() as u64, mode),
        audio_secs,
        left_level: 0.0,
        right_level: 0.0,
        render_ui: true,
        emit_event: true,
    })?;

    Ok(CaptureResult {
        samples,
        elapsed,
        audio_secs,
        stop_reason,
    })
}

fn to_i16(sample: f32) -> i16 {
    (sample * 32767.0) as i16
}

fn peak_level(samples: &[f32]) -> f32 {
    samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max)
        .clamp(0.0, 1.0)
}

fn samples_to_audio_secs(samples_len: u64, sample_rate: u32, mode: AudioMode) -> f64 {
    let divisor = match mode {
        AudioMode::Dual => sample_rate as f64 * 2.0,
        AudioMode::Input | AudioMode::Output => sample_rate as f64,
    };
    samples_len as f64 / divisor
}

fn audio_frame_count(samples_len: u64, mode: AudioMode) -> u64 {
    match mode {
        AudioMode::Dual => samples_len / 2,
        AudioMode::Input | AudioMode::Output => samples_len,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use hypr_audio::{CaptureFrame, CaptureStream, Error};
    use tokio_stream::iter;

    #[test]
    fn peak_level_clamps_to_unit_interval() {
        assert_eq!(peak_level(&[0.1, -0.4, 0.2]), 0.4);
        assert_eq!(peak_level(&[2.0]), 1.0);
    }

    #[test]
    fn dual_audio_frame_count_uses_stereo_pairs() {
        assert_eq!(audio_frame_count(10, AudioMode::Dual), 5);
        assert_eq!(audio_frame_count(10, AudioMode::Input), 10);
    }

    struct TestAudio {
        frames: Vec<Result<CaptureFrame, Error>>,
    }

    impl AudioProvider for TestAudio {
        fn open_capture(&self, _config: CaptureConfig) -> Result<CaptureStream, Error> {
            Ok(CaptureStream::new(iter(self.frames.clone())))
        }

        fn open_speaker_capture(
            &self,
            _sample_rate: u32,
            _chunk_size: usize,
        ) -> Result<CaptureStream, Error> {
            Ok(CaptureStream::new(iter(self.frames.clone())))
        }

        fn open_mic_capture(
            &self,
            _device: Option<String>,
            _sample_rate: u32,
            _chunk_size: usize,
        ) -> Result<CaptureStream, Error> {
            Ok(CaptureStream::new(iter(self.frames.clone())))
        }

        fn default_device_name(&self) -> String {
            "test".to_string()
        }

        fn list_mic_devices(&self) -> Vec<String> {
            vec!["test".to_string()]
        }

        fn play_silence(&self) -> std::sync::mpsc::Sender<()> {
            let (tx, _rx) = std::sync::mpsc::channel();
            tx
        }

        fn play_bytes(&self, _bytes: &'static [u8]) -> std::sync::mpsc::Sender<()> {
            let (tx, _rx) = std::sync::mpsc::channel();
            tx
        }

        fn probe_mic(&self, _device: Option<String>) -> Result<(), Error> {
            Ok(())
        }

        fn probe_speaker(&self) -> Result<(), Error> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn capture_returns_samples_and_eof_reason() {
        let audio = TestAudio {
            frames: vec![Ok(CaptureFrame {
                raw_mic: Arc::from([0.25_f32, -0.25, 0.5, -0.5]),
                raw_speaker: Arc::from([]),
                aec_mic: None,
            })],
        };
        let mut updates = Vec::new();

        let result = capture(&audio, AudioMode::Input, 16_000, 4, |progress| {
            updates.push(progress.sample_count);
            Ok(())
        })
        .await
        .unwrap();

        assert_eq!(result.stop_reason.as_str(), "eof");
        assert_eq!(result.samples.len(), 4);
        assert!(!updates.is_empty());
    }
}

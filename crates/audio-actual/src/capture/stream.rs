use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::{panic::AssertUnwindSafe, panic::catch_unwind};

use futures_util::{Stream, StreamExt};
use hypr_aec::AEC;
use hypr_audio_sync::{SyncProbe, SyncProbeConfig, SyncProbeEvent, SyncProbeState};
use hypr_resampler::ResampleExtDynamicNew;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

use hypr_audio::{CaptureFrame, CaptureStream, Error};

use crate::mic::MicInput;
use crate::speaker::SpeakerInput;

use super::joiner::Joiner;

pub(crate) type ChunkStream =
    Pin<Box<dyn Stream<Item = Result<Vec<f32>, hypr_resampler::Error>> + Send>>;

const AUDIO_SYNC_PROBE_ENV: &str = "AUDIO_SYNC_PROBE";
const AEC_MAX_REFERENCE_LAG_MS: u32 = 100;
const AEC_MIN_REFERENCE_RMS: f32 = 1e-4;
const AEC_MIN_MIC_RMS: f32 = 1e-4;
const AEC_MIN_REFERENCE_CORRELATION: f32 = 0.12;
const AEC_MAX_LINEAR_GAIN: f32 = 1.25;
const AEC_LINEAR_GAIN_SMOOTHING: f32 = 0.12;
const AEC_DOUBLE_TALK_RESIDUAL_RATIO: f32 = 0.08;
const AEC_ROBUST_GAIN_SEGMENTS: usize = 8;
const AEC_ROBUST_GAIN_MIN_SEGMENTS: usize = 3;

struct CaptureStreamInner {
    inner: ReceiverStream<Result<CaptureFrame, Error>>,
    cancel_token: CancellationToken,
    task: JoinHandle<()>,
}

impl Stream for CaptureStreamInner {
    type Item = Result<CaptureFrame, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for CaptureStreamInner {
    fn drop(&mut self) {
        self.cancel_token.cancel();
        self.task.abort();
    }
}

pub(crate) fn setup_mic_stream(
    sample_rate: u32,
    chunk_size: usize,
    mic_device: Option<String>,
) -> Result<ChunkStream, Error> {
    let mic = MicInput::new(mic_device).map_err(|_| Error::MicOpenFailed)?;
    mic.stream()
        .resampled_chunks(sample_rate, chunk_size)
        .map(|stream| Box::pin(stream) as ChunkStream)
        .map_err(|_| Error::MicStreamSetupFailed)
}

pub(crate) fn setup_speaker_stream(
    sample_rate: u32,
    chunk_size: usize,
) -> Result<ChunkStream, Error> {
    let speaker = SpeakerInput::new().map_err(|_| Error::SpeakerStreamSetupFailed)?;
    speaker
        .stream()
        .map_err(|_| Error::SpeakerStreamSetupFailed)?
        .resampled_chunks(sample_rate, chunk_size)
        .map(|stream| Box::pin(stream) as ChunkStream)
        .map_err(|_| Error::SpeakerStreamSetupFailed)
}

pub(crate) fn open_dual(
    sample_rate: u32,
    mic_stream: ChunkStream,
    speaker_stream: ChunkStream,
    enable_aec: bool,
) -> CaptureStream {
    let cancel_token = CancellationToken::new();
    let (tx, rx) = tokio::sync::mpsc::channel(32);
    let task = tokio::spawn(run_dual_loop(
        tx,
        cancel_token.clone(),
        sample_rate,
        enable_aec,
        mic_stream,
        speaker_stream,
    ));

    CaptureStream::new(CaptureStreamInner {
        inner: ReceiverStream::new(rx),
        cancel_token,
        task,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureSide {
    Mic,
    Speaker,
}

pub(crate) fn open_single(chunk_stream: ChunkStream, side: CaptureSide) -> CaptureStream {
    let cancel_token = CancellationToken::new();
    let (tx, rx) = tokio::sync::mpsc::channel(32);
    let task = tokio::spawn(run_single_loop(
        tx,
        cancel_token.clone(),
        chunk_stream,
        side,
    ));

    CaptureStream::new(CaptureStreamInner {
        inner: ReceiverStream::new(rx),
        cancel_token,
        task,
    })
}

enum StreamResult {
    Continue,
    Stop,
    Failed(Error),
}

async fn run_dual_loop(
    tx: tokio::sync::mpsc::Sender<Result<CaptureFrame, Error>>,
    cancel_token: CancellationToken,
    sample_rate: u32,
    enable_aec: bool,
    mut mic_stream: ChunkStream,
    mut speaker_stream: ChunkStream,
) {
    let mut joiner = Joiner::new();
    let mut aec = if enable_aec { build_aec() } else { None };
    let mut linear_echo_gain = None;
    let mut aec_reference = if aec.is_some() {
        Some(AecReferenceAligner::new(sample_rate))
    } else {
        None
    };

    loop {
        let result = tokio::select! {
            _ = cancel_token.cancelled() => StreamResult::Stop,
            item = mic_stream.next() => {
                handle_stream_item(item, CaptureSide::Mic, &mut joiner)
            }
            item = speaker_stream.next() => {
                handle_stream_item(item, CaptureSide::Speaker, &mut joiner)
            }
        };

        match result {
            StreamResult::Continue => {
                while let Some((raw_mic, raw_speaker)) = joiner.pop_pair() {
                    let raw_mic = Arc::<[f32]>::from(raw_mic);
                    let raw_speaker = Arc::<[f32]>::from(raw_speaker);
                    let aec_reference_speaker = aec_reference
                        .as_mut()
                        .map(|aligner| aligner.align(&raw_speaker, &raw_mic))
                        .unwrap_or_else(|| Arc::clone(&raw_speaker));
                    let aec_mic = process_aec(
                        &mut aec,
                        &mut linear_echo_gain,
                        &raw_mic,
                        &aec_reference_speaker,
                    );
                    if tx
                        .send(Ok(CaptureFrame {
                            raw_mic,
                            raw_speaker,
                            aec_mic,
                        }))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
            StreamResult::Stop => return,
            StreamResult::Failed(err) => {
                let _ = tx.send(Err(err)).await;
                return;
            }
        }
    }
}

struct AecReferenceAligner {
    probe: SyncProbe,
    delay_line: SampleDelayLine,
    last_delay_samples: usize,
    last_logged_state: Option<SyncProbeState>,
    last_logged_stable_lag_samples: Option<isize>,
    log_probe_events: bool,
}

impl AecReferenceAligner {
    fn new(sample_rate: u32) -> Self {
        let max_lag_samples = ((sample_rate as usize) * (AEC_MAX_REFERENCE_LAG_MS as usize)) / 1000;
        let mut config = SyncProbeConfig::new(sample_rate);
        config.max_lag_samples = max_lag_samples.max(config.max_lag_samples);
        let max_delay_samples = config.max_lag_samples;

        Self {
            probe: SyncProbe::new(config),
            delay_line: SampleDelayLine::new(max_delay_samples),
            last_delay_samples: 0,
            last_logged_state: None,
            last_logged_stable_lag_samples: None,
            log_probe_events: std::env::var(AUDIO_SYNC_PROBE_ENV).ok().as_deref() == Some("1"),
        }
    }

    fn align(&mut self, raw_speaker: &[f32], raw_mic: &[f32]) -> Arc<[f32]> {
        let observed = catch_unwind(AssertUnwindSafe(|| {
            self.probe.observe(raw_speaker, raw_mic)
        }));
        let event = match observed {
            Ok(event) => event,
            Err(_) => {
                tracing::error!("audio_sync_probe_panicked");
                None
            }
        };

        if let Some(event) = event {
            self.update_delay(&event);
            if self.log_probe_events {
                self.log_probe_event(event);
            }
        }

        Arc::<[f32]>::from(
            self.delay_line
                .process(raw_speaker, self.last_delay_samples),
        )
    }

    fn update_delay(&mut self, event: &SyncProbeEvent) {
        let snapshot = event.snapshot();
        let next_delay = if matches!(
            snapshot.state,
            SyncProbeState::Locked | SyncProbeState::Holdover
        ) {
            snapshot
                .stable_lag_samples
                .filter(|lag| *lag > 0)
                .map(|lag| lag as usize)
                .unwrap_or(0)
        } else {
            0
        };

        if next_delay != self.last_delay_samples {
            tracing::info!(
                previous_delay_samples = self.last_delay_samples,
                delay_samples = next_delay,
                delay_ms = next_delay as f32 / self.probe.config().sample_rate as f32 * 1000.0,
                state = ?snapshot.state,
                "aec_reference_delay_changed"
            );
            self.last_delay_samples = next_delay;
        }
    }

    fn log_probe_event(&mut self, event: SyncProbeEvent) {
        let snapshot = event.snapshot();
        let should_log = self.last_logged_state != Some(snapshot.state)
            || self.last_logged_stable_lag_samples != snapshot.stable_lag_samples;

        if !should_log {
            return;
        }

        match event {
            SyncProbeEvent::Measured(measurement) => {
                tracing::info!(
                    capture_time_sec = measurement.capture_time_sec,
                    state = ?measurement.snapshot.state,
                    stable_lag_samples = measurement.snapshot.stable_lag_samples,
                    candidate_lag_samples = measurement.snapshot.candidate_lag_samples,
                    accepted_window_count = measurement.snapshot.accepted_window_count,
                    confidence = measurement.snapshot.confidence,
                    peak_ratio = measurement.estimate.peak_ratio,
                    distinctiveness = measurement.estimate.distinctiveness,
                    drift_ppm = measurement.trend.drift_ppm,
                    "audio_sync_probe"
                );
            }
            SyncProbeEvent::SkippedLowConfidence(skip) => {
                tracing::info!(
                    capture_time_sec = skip.capture_time_sec,
                    state = ?skip.snapshot.state,
                    stable_lag_samples = skip.snapshot.stable_lag_samples,
                    candidate_lag_samples = skip.snapshot.candidate_lag_samples,
                    accepted_window_count = skip.snapshot.accepted_window_count,
                    confidence = skip.snapshot.confidence,
                    reason = ?skip.reason,
                    peak_ratio = skip.estimate.peak_ratio,
                    distinctiveness = skip.estimate.distinctiveness,
                    "audio_sync_probe"
                );
            }
            SyncProbeEvent::SkippedLowEnergy(skip) => {
                tracing::info!(
                    capture_time_sec = skip.capture_time_sec,
                    state = ?skip.snapshot.state,
                    stable_lag_samples = skip.snapshot.stable_lag_samples,
                    accepted_window_count = skip.snapshot.accepted_window_count,
                    reference_rms = skip.reference_rms,
                    observed_rms = skip.observed_rms,
                    "audio_sync_probe"
                );
            }
        }

        self.last_logged_state = Some(snapshot.state);
        self.last_logged_stable_lag_samples = snapshot.stable_lag_samples;
    }
}

struct SampleDelayLine {
    history: VecDeque<f32>,
    max_delay_samples: usize,
}

impl SampleDelayLine {
    fn new(max_delay_samples: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_delay_samples + 1),
            max_delay_samples,
        }
    }

    fn process(&mut self, input: &[f32], delay_samples: usize) -> Vec<f32> {
        let delay_samples = delay_samples.min(self.max_delay_samples);
        let mut output = Vec::with_capacity(input.len());

        for &sample in input {
            self.history.push_back(sample);
            let delayed = self
                .history
                .len()
                .checked_sub(delay_samples + 1)
                .and_then(|idx| self.history.get(idx))
                .copied()
                .unwrap_or(0.0);
            output.push(delayed);

            while self.history.len() > self.max_delay_samples + 1 {
                self.history.pop_front();
            }
        }

        output
    }
}

async fn run_single_loop(
    tx: tokio::sync::mpsc::Sender<Result<CaptureFrame, Error>>,
    cancel_token: CancellationToken,
    mut chunk_stream: ChunkStream,
    side: CaptureSide,
) {
    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => return,
            item = chunk_stream.next() => {
                match item {
                    Some(Ok(data)) => {
                        let data = Arc::<[f32]>::from(data);
                        let silence = Arc::<[f32]>::from(vec![0.0f32; data.len()]);
                        let frame = match side {
                            CaptureSide::Mic => CaptureFrame {
                                raw_mic: data,
                                raw_speaker: silence,
                                aec_mic: None,
                            },
                            CaptureSide::Speaker => CaptureFrame {
                                raw_mic: silence,
                                raw_speaker: data,
                                aec_mic: None,
                            },
                        };
                        if tx.send(Ok(frame)).await.is_err() {
                            return;
                        }
                    }
                    Some(Err(_)) => {
                        let err = match side {
                            CaptureSide::Mic => Error::MicResampleFailed,
                            CaptureSide::Speaker => Error::SpeakerResampleFailed,
                        };
                        let _ = tx.send(Err(err)).await;
                        return;
                    }
                    None => {
                        let err = match side {
                            CaptureSide::Mic => Error::MicStreamEnded,
                            CaptureSide::Speaker => Error::SpeakerStreamEnded,
                        };
                        let _ = tx.send(Err(err)).await;
                        return;
                    }
                }
            }
        }
    }
}

fn handle_stream_item(
    item: Option<Result<Vec<f32>, hypr_resampler::Error>>,
    side: CaptureSide,
    joiner: &mut Joiner,
) -> StreamResult {
    match item {
        Some(Ok(data)) => {
            match side {
                CaptureSide::Mic => joiner.push_mic(data),
                CaptureSide::Speaker => joiner.push_speaker(data),
            }
            StreamResult::Continue
        }
        Some(Err(_)) => StreamResult::Failed(match side {
            CaptureSide::Mic => Error::MicResampleFailed,
            CaptureSide::Speaker => Error::SpeakerResampleFailed,
        }),
        None => StreamResult::Failed(match side {
            CaptureSide::Mic => Error::MicStreamEnded,
            CaptureSide::Speaker => Error::SpeakerStreamEnded,
        }),
    }
}

fn build_aec() -> Option<AEC> {
    AEC::new()
        .map_err(|error| tracing::warn!(error.message = ?error, "aec_init_failed"))
        .ok()
}

fn process_aec(
    aec: &mut Option<AEC>,
    linear_echo_gain: &mut Option<f32>,
    mic: &[f32],
    speaker: &[f32],
) -> Option<Arc<[f32]>> {
    let processor = aec.as_mut()?;
    match processor.process_streaming(mic, speaker) {
        Ok(processed) => Some(Arc::<[f32]>::from(cancel_linear_echo(
            speaker,
            processed,
            linear_echo_gain,
        ))),
        Err(error) => {
            tracing::warn!(error.message = ?error, "aec_failed");
            None
        }
    }
}

fn cancel_linear_echo(
    speaker: &[f32],
    processed: Vec<f32>,
    linear_echo_gain: &mut Option<f32>,
) -> Vec<f32> {
    let len = speaker.len().min(processed.len());
    if len == 0 {
        return processed;
    }

    let mut processed_energy = 0.0;
    let mut speaker_energy = 0.0;
    let mut cross_energy = 0.0;
    for idx in 0..len {
        let processed_sample = processed[idx];
        let speaker_sample = speaker[idx];
        processed_energy += processed_sample * processed_sample;
        speaker_energy += speaker_sample * speaker_sample;
        cross_energy += processed_sample * speaker_sample;
    }

    let len_f32 = len as f32;
    let processed_rms = (processed_energy / len_f32).sqrt();
    let speaker_rms = (speaker_energy / len_f32).sqrt();
    if processed_rms < AEC_MIN_MIC_RMS || speaker_rms < AEC_MIN_REFERENCE_RMS {
        return processed;
    }

    let correlation = cross_energy.abs() / (processed_energy * speaker_energy).sqrt().max(1e-6);
    if correlation < AEC_MIN_REFERENCE_CORRELATION {
        return processed;
    }

    let instantaneous_gain =
        (cross_energy / speaker_energy.max(1e-6)).clamp(-AEC_MAX_LINEAR_GAIN, AEC_MAX_LINEAR_GAIN);
    let residual_energy =
        (processed_energy - (cross_energy * cross_energy / speaker_energy.max(1e-6))).max(0.0);
    let residual_ratio = (residual_energy / len_f32).sqrt() / processed_rms.max(1e-6);
    let measured_gain = if residual_ratio > AEC_DOUBLE_TALK_RESIDUAL_RATIO {
        segmented_trimmed_gain(&processed, speaker, len, instantaneous_gain)
    } else {
        instantaneous_gain
    };
    let smoothed_gain = linear_echo_gain
        .map(|gain| gain + (measured_gain - gain) * AEC_LINEAR_GAIN_SMOOTHING)
        .unwrap_or(measured_gain);
    *linear_echo_gain = Some(smoothed_gain);

    let gain = if residual_ratio > AEC_DOUBLE_TALK_RESIDUAL_RATIO {
        smoothed_gain
    } else {
        measured_gain
    };

    let mut output = Vec::with_capacity(processed.len());
    output.extend(processed.iter().zip(speaker).take(len).map(
        |(processed_sample, speaker_sample)| {
            (processed_sample - gain * speaker_sample).clamp(-1.0, 1.0)
        },
    ));
    output.extend_from_slice(&processed[len..]);
    output
}

fn segmented_trimmed_gain(
    processed: &[f32],
    speaker: &[f32],
    len: usize,
    initial_gain: f32,
) -> f32 {
    let segment_len = (len / AEC_ROBUST_GAIN_SEGMENTS).max(1);
    let mut gains = Vec::with_capacity(AEC_ROBUST_GAIN_SEGMENTS);
    let mut start = 0;
    while start < len {
        let end = (start + segment_len).min(len);
        let mut processed_energy = 0.0;
        let mut speaker_energy = 0.0;
        let mut cross_energy = 0.0;
        for idx in start..end {
            let processed_sample = processed[idx];
            let speaker_sample = speaker[idx];
            processed_energy += processed_sample * processed_sample;
            speaker_energy += speaker_sample * speaker_sample;
            cross_energy += processed_sample * speaker_sample;
        }

        if processed_energy > 1e-8 && speaker_energy > 1e-8 {
            let correlation =
                cross_energy.abs() / (processed_energy * speaker_energy).sqrt().max(1e-6);
            if correlation >= AEC_MIN_REFERENCE_CORRELATION {
                let gain = (cross_energy / speaker_energy.max(1e-6))
                    .clamp(-AEC_MAX_LINEAR_GAIN, AEC_MAX_LINEAR_GAIN);
                gains.push((gain, speaker_energy));
            }
        }

        start = end;
    }

    if gains.len() < AEC_ROBUST_GAIN_MIN_SEGMENTS {
        return initial_gain;
    }

    gains.sort_by(|left, right| left.0.total_cmp(&right.0));
    let trim = if gains.len() >= 6 { gains.len() / 6 } else { 0 };
    let kept = &gains[trim..gains.len() - trim];
    let weighted_gain = kept.iter().map(|(gain, energy)| gain * energy).sum::<f32>();
    let kept_energy = kept.iter().map(|(_, energy)| energy).sum::<f32>();
    if kept_energy <= 1e-8 {
        return initial_gain;
    }

    (weighted_gain / kept_energy).clamp(-AEC_MAX_LINEAR_GAIN, AEC_MAX_LINEAR_GAIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_aec_returns_instance() {
        let aec = build_aec();
        assert!(aec.is_some());
    }

    #[test]
    fn process_aec_returns_output_when_enabled() {
        let mut aec = build_aec();
        let mut linear_echo_gain = None;
        let mic = Arc::<[f32]>::from(vec![0.1_f32; 160]);
        let speaker = Arc::<[f32]>::from(vec![0.2_f32; 160]);

        let processed = process_aec(&mut aec, &mut linear_echo_gain, &mic, &speaker);
        assert_eq!(processed.as_ref().map(|data| data.len()), Some(160));
    }

    #[test]
    fn sample_delay_line_outputs_current_samples_with_zero_delay() {
        let mut delay = SampleDelayLine::new(4);

        let output = delay.process(&[1.0, 2.0, 3.0], 0);

        assert_eq!(output, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn sample_delay_line_outputs_delayed_samples() {
        let mut delay = SampleDelayLine::new(4);

        let first = delay.process(&[1.0, 2.0, 3.0], 2);
        let second = delay.process(&[4.0, 5.0], 2);

        assert_eq!(first, vec![0.0, 0.0, 1.0]);
        assert_eq!(second, vec![2.0, 3.0]);
    }

    #[test]
    fn sample_delay_line_clamps_to_max_delay() {
        let mut delay = SampleDelayLine::new(2);

        let first = delay.process(&[1.0, 2.0, 3.0], 10);
        let second = delay.process(&[4.0], 10);

        assert_eq!(first, vec![0.0, 0.0, 1.0]);
        assert_eq!(second, vec![2.0]);
    }
}

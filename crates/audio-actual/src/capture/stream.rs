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
    let mut sync_probe = ObserveOnlySyncProbe::from_env(sample_rate);

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
                    if let Some(probe) = &mut sync_probe {
                        probe.observe(&raw_mic, &raw_speaker);
                    }
                    let aec_mic = process_aec(&mut aec, &raw_mic, &raw_speaker);
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

struct ObserveOnlySyncProbe {
    probe: SyncProbe,
    last_logged_state: Option<SyncProbeState>,
    last_logged_stable_lag_samples: Option<isize>,
}

impl ObserveOnlySyncProbe {
    fn from_env(sample_rate: u32) -> Option<Self> {
        if std::env::var(AUDIO_SYNC_PROBE_ENV).ok().as_deref() != Some("1") {
            return None;
        }

        Some(Self {
            probe: SyncProbe::new(SyncProbeConfig::new(sample_rate)),
            last_logged_state: None,
            last_logged_stable_lag_samples: None,
        })
    }

    fn observe(&mut self, raw_mic: &[f32], raw_speaker: &[f32]) {
        let observed = catch_unwind(AssertUnwindSafe(|| {
            self.probe.observe(raw_speaker, raw_mic)
        }));
        let Some(event) = (match observed {
            Ok(event) => event,
            Err(_) => {
                tracing::error!("audio_sync_probe_panicked");
                return;
            }
        }) else {
            return;
        };

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

fn process_aec(aec: &mut Option<AEC>, mic: &[f32], speaker: &[f32]) -> Option<Arc<[f32]>> {
    let processor = aec.as_mut()?;
    match processor.process_streaming(mic, speaker) {
        Ok(processed) => Some(Arc::<[f32]>::from(processed)),
        Err(error) => {
            tracing::warn!(error.message = ?error, "aec_failed");
            None
        }
    }
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
        let mic = Arc::<[f32]>::from(vec![0.1_f32; 160]);
        let speaker = Arc::<[f32]>::from(vec![0.2_f32; 160]);

        let processed = process_aec(&mut aec, &mic, &speaker);
        assert_eq!(processed.as_ref().map(|data| data.len()), Some(160));
    }
}

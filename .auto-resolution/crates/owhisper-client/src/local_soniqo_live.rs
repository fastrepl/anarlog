use std::time::Duration;

use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use owhisper_interface::stream::StreamResponse;
use owhisper_interface::{ControlMessage, MixedMessage};
use tokio_stream::wrappers::ReceiverStream;

use crate::{FinalizeHandle, ListenClientDualInput, ListenClientInput};

const FLUSH_INTERVAL: Duration = Duration::from_millis(250);
const ECHO_GATE_MIN_SAMPLES: usize = 512;
const ECHO_GATE_MAX_LAG_SAMPLES: isize = 1600;
const ECHO_GATE_LAG_STEP_SAMPLES: isize = 80;
const ECHO_GATE_MIN_MIC_RMS: f32 = 0.0025;
const ECHO_GATE_MIN_SPEAKER_RMS: f32 = 0.01;
const ECHO_GATE_MIN_CORRELATION: f32 = 0.55;
const ECHO_GATE_MAX_RESIDUAL_RATIO: f32 = 0.45;

pub type LocalSoniqoLiveStream = ReceiverStream<Result<StreamResponse, LocalSoniqoLiveError>>;

#[derive(Debug, thiserror::Error)]
pub enum LocalSoniqoLiveError {
    #[error("soniqo_live_start_join_failed: {0}")]
    StartJoin(String),
    #[error("soniqo_live_start_failed: {0}")]
    Start(String),
    #[error("soniqo_live_append_join_failed: {0}")]
    AppendJoin(String),
    #[error("soniqo_live_append_failed: {0}")]
    Append(String),
    #[error("soniqo_live_finalize_join_failed: {0}")]
    FinalizeJoin(String),
    #[error("soniqo_live_finalize_failed: {0}")]
    Finalize(String),
}

pub struct LocalSoniqoLiveClient {
    model: hypr_transcribe_soniqo::SoniqoModel,
}

impl LocalSoniqoLiveClient {
    pub fn new(model: hypr_transcribe_soniqo::SoniqoModel) -> Self {
        Self { model }
    }

    pub async fn from_realtime_audio_single(
        self,
        stream: impl Stream<Item = ListenClientInput> + Send + Unpin + 'static,
        source: hypr_transcribe_soniqo::TranscriptSource,
    ) -> Result<(LocalSoniqoLiveStream, LocalSoniqoLiveHandle), LocalSoniqoLiveError> {
        let session = start_session(self.model).await?;
        let (response_tx, response_rx) = tokio::sync::mpsc::channel(32);
        let (finalize_tx, finalize_rx) = tokio::sync::mpsc::channel(1);

        tokio::spawn(run_single(
            session,
            self.model,
            source,
            stream,
            response_tx,
            finalize_rx,
        ));

        Ok((
            ReceiverStream::new(response_rx),
            LocalSoniqoLiveHandle {
                finalize_tx,
                expected_finalize_count: 1,
            },
        ))
    }

    pub async fn from_realtime_audio_dual(
        self,
        stream: impl Stream<Item = ListenClientDualInput> + Send + Unpin + 'static,
    ) -> Result<(LocalSoniqoLiveStream, LocalSoniqoLiveHandle), LocalSoniqoLiveError> {
        let session = start_session(self.model).await?;
        let (response_tx, response_rx) = tokio::sync::mpsc::channel(32);
        let (finalize_tx, finalize_rx) = tokio::sync::mpsc::channel(1);

        tokio::spawn(run_dual(
            session,
            self.model,
            stream,
            response_tx,
            finalize_rx,
        ));

        Ok((
            ReceiverStream::new(response_rx),
            LocalSoniqoLiveHandle {
                finalize_tx,
                expected_finalize_count: 2,
            },
        ))
    }
}

pub struct LocalSoniqoLiveHandle {
    finalize_tx: tokio::sync::mpsc::Sender<()>,
    expected_finalize_count: usize,
}

impl FinalizeHandle for LocalSoniqoLiveHandle {
    async fn finalize(&self) {
        let _ = self.finalize_tx.send(()).await;
    }

    fn expected_finalize_count(&self) -> usize {
        self.expected_finalize_count
    }
}

async fn run_single(
    session: hypr_transcribe_soniqo::LiveTranscriptionSession,
    model: hypr_transcribe_soniqo::SoniqoModel,
    source: hypr_transcribe_soniqo::TranscriptSource,
    mut stream: impl Stream<Item = ListenClientInput> + Send + Unpin + 'static,
    response_tx: tokio::sync::mpsc::Sender<Result<StreamResponse, LocalSoniqoLiveError>>,
    mut finalize_rx: tokio::sync::mpsc::Receiver<()>,
) {
    let mut session = session;
    let mut buffer = Vec::<f32>::new();
    let mut cursor = 0.0;
    let mut interval = tokio::time::interval(FLUSH_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = finalize_rx.recv() => {
                let result = finalize_single(session, model, source, &mut buffer, &mut cursor, &response_tx).await;
                stop_after_finalize(result).await;
                return;
            }
            maybe_msg = stream.next() => {
                let Some(msg) = maybe_msg else {
                    stop_session(session).await;
                    return;
                };

                match msg {
                    MixedMessage::Audio(audio) => {
                        buffer.extend(i16_bytes_to_f32(&audio));
                    }
                    MixedMessage::Control(ControlMessage::CloseStream | ControlMessage::Finalize) => {
                        let result = finalize_single(session, model, source, &mut buffer, &mut cursor, &response_tx).await;
                        stop_after_finalize(result).await;
                        return;
                    }
                    MixedMessage::Control(ControlMessage::KeepAlive) => {}
                }
            }
            _ = interval.tick() => {
                match flush_buffer(session, model, source, &mut buffer, &mut cursor, &response_tx).await {
                    Ok(next_session) => session = next_session,
                    Err(error) => {
                        let _ = response_tx.send(Err(error)).await;
                        return;
                    }
                }
            }
        }
    }
}

async fn run_dual(
    session: hypr_transcribe_soniqo::LiveTranscriptionSession,
    model: hypr_transcribe_soniqo::SoniqoModel,
    mut stream: impl Stream<Item = ListenClientDualInput> + Send + Unpin + 'static,
    response_tx: tokio::sync::mpsc::Sender<Result<StreamResponse, LocalSoniqoLiveError>>,
    mut finalize_rx: tokio::sync::mpsc::Receiver<()>,
) {
    let mut session = session;
    let mut mic_buffer = Vec::<f32>::new();
    let mut spk_buffer = Vec::<f32>::new();
    let mut mic_cursor = 0.0;
    let mut spk_cursor = 0.0;
    let mut interval = tokio::time::interval(FLUSH_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = finalize_rx.recv() => {
                let result = finalize_dual(session, model, &mut mic_buffer, &mut spk_buffer, &mut mic_cursor, &mut spk_cursor, &response_tx).await;
                stop_after_finalize(result).await;
                return;
            }
            maybe_msg = stream.next() => {
                let Some(msg) = maybe_msg else {
                    stop_session(session).await;
                    return;
                };

                match msg {
                    MixedMessage::Audio((mic, spk)) => {
                        let mut mic_samples = i16_bytes_to_f32(&mic);
                        let spk_samples = i16_bytes_to_f32(&spk);
                        if suppress_echo_dominant_mic(&mut mic_samples, &spk_samples) {
                            tracing::debug!("soniqo_echo_dominant_mic_chunk_suppressed");
                        }
                        mic_buffer.extend(mic_samples);
                        spk_buffer.extend(spk_samples);
                    }
                    MixedMessage::Control(ControlMessage::CloseStream | ControlMessage::Finalize) => {
                        let result = finalize_dual(session, model, &mut mic_buffer, &mut spk_buffer, &mut mic_cursor, &mut spk_cursor, &response_tx).await;
                        stop_after_finalize(result).await;
                        return;
                    }
                    MixedMessage::Control(ControlMessage::KeepAlive) => {}
                }
            }
            _ = interval.tick() => {
                match flush_buffer(
                    session,
                    model,
                    hypr_transcribe_soniqo::TranscriptSource::Microphone,
                    &mut mic_buffer,
                    &mut mic_cursor,
                    &response_tx,
                )
                .await
                {
                    Ok(next_session) => session = next_session,
                    Err(error) => {
                        let _ = response_tx.send(Err(error)).await;
                        return;
                    }
                }

                match flush_buffer(
                    session,
                    model,
                    hypr_transcribe_soniqo::TranscriptSource::System,
                    &mut spk_buffer,
                    &mut spk_cursor,
                    &response_tx,
                )
                .await
                {
                    Ok(next_session) => session = next_session,
                    Err(error) => {
                        let _ = response_tx.send(Err(error)).await;
                        return;
                    }
                }
            }
        }
    }
}

async fn finalize_single(
    session: hypr_transcribe_soniqo::LiveTranscriptionSession,
    model: hypr_transcribe_soniqo::SoniqoModel,
    source: hypr_transcribe_soniqo::TranscriptSource,
    buffer: &mut Vec<f32>,
    cursor: &mut f64,
    response_tx: &tokio::sync::mpsc::Sender<Result<StreamResponse, LocalSoniqoLiveError>>,
) -> Result<hypr_transcribe_soniqo::LiveTranscriptionSession, LocalSoniqoLiveError> {
    let session = flush_buffer(session, model, source, buffer, cursor, response_tx).await?;
    finalize_source(session, model, source, *cursor, response_tx).await
}

async fn finalize_dual(
    session: hypr_transcribe_soniqo::LiveTranscriptionSession,
    model: hypr_transcribe_soniqo::SoniqoModel,
    mic_buffer: &mut Vec<f32>,
    spk_buffer: &mut Vec<f32>,
    mic_cursor: &mut f64,
    spk_cursor: &mut f64,
    response_tx: &tokio::sync::mpsc::Sender<Result<StreamResponse, LocalSoniqoLiveError>>,
) -> Result<hypr_transcribe_soniqo::LiveTranscriptionSession, LocalSoniqoLiveError> {
    let session = flush_buffer(
        session,
        model,
        hypr_transcribe_soniqo::TranscriptSource::Microphone,
        mic_buffer,
        mic_cursor,
        response_tx,
    )
    .await?;
    let session = flush_buffer(
        session,
        model,
        hypr_transcribe_soniqo::TranscriptSource::System,
        spk_buffer,
        spk_cursor,
        response_tx,
    )
    .await?;
    let session = finalize_source(
        session,
        model,
        hypr_transcribe_soniqo::TranscriptSource::Microphone,
        *mic_cursor,
        response_tx,
    )
    .await?;
    finalize_source(
        session,
        model,
        hypr_transcribe_soniqo::TranscriptSource::System,
        *spk_cursor,
        response_tx,
    )
    .await
}

async fn stop_after_finalize(
    result: Result<hypr_transcribe_soniqo::LiveTranscriptionSession, LocalSoniqoLiveError>,
) {
    match result {
        Ok(session) => stop_session(session).await,
        Err(error) => tracing::warn!(error.message = %error, "soniqo_live_finalize_failed"),
    }
}

async fn start_session(
    model: hypr_transcribe_soniqo::SoniqoModel,
) -> Result<hypr_transcribe_soniqo::LiveTranscriptionSession, LocalSoniqoLiveError> {
    tokio::task::spawn_blocking(move || {
        hypr_transcribe_soniqo::LiveTranscriptionSession::start(model)
    })
    .await
    .map_err(|error| LocalSoniqoLiveError::StartJoin(error.to_string()))?
    .map_err(|error| LocalSoniqoLiveError::Start(error.to_string()))
}

async fn stop_session(session: hypr_transcribe_soniqo::LiveTranscriptionSession) {
    let _ = tokio::task::spawn_blocking(move || session.stop()).await;
}

async fn flush_buffer(
    session: hypr_transcribe_soniqo::LiveTranscriptionSession,
    model: hypr_transcribe_soniqo::SoniqoModel,
    source: hypr_transcribe_soniqo::TranscriptSource,
    buffer: &mut Vec<f32>,
    cursor: &mut f64,
    response_tx: &tokio::sync::mpsc::Sender<Result<StreamResponse, LocalSoniqoLiveError>>,
) -> Result<hypr_transcribe_soniqo::LiveTranscriptionSession, LocalSoniqoLiveError> {
    if buffer.is_empty() {
        return Ok(session);
    }

    let samples = std::mem::take(buffer);
    let duration = samples.len() as f64 / 16_000.0;
    let start = *cursor;
    *cursor += duration;

    append_source(
        session,
        model,
        source,
        samples,
        start,
        duration,
        response_tx,
    )
    .await
}

async fn append_source(
    session: hypr_transcribe_soniqo::LiveTranscriptionSession,
    model: hypr_transcribe_soniqo::SoniqoModel,
    source: hypr_transcribe_soniqo::TranscriptSource,
    samples: Vec<f32>,
    start: f64,
    duration: f64,
    response_tx: &tokio::sync::mpsc::Sender<Result<StreamResponse, LocalSoniqoLiveError>>,
) -> Result<hypr_transcribe_soniqo::LiveTranscriptionSession, LocalSoniqoLiveError> {
    let joined = tokio::task::spawn_blocking(move || {
        let mut session = session;
        let result = session.append(source, &samples);
        (session, result)
    })
    .await
    .map_err(|error| LocalSoniqoLiveError::AppendJoin(error.to_string()))?;

    let (session, partials) = joined;
    let partials = partials.map_err(|error| LocalSoniqoLiveError::Append(error.to_string()))?;

    for partial in partials {
        let response = partial.into_stream_response(model, start, duration);
        if response_tx.send(Ok(response)).await.is_err() {
            break;
        }
    }

    Ok(session)
}

async fn finalize_source(
    session: hypr_transcribe_soniqo::LiveTranscriptionSession,
    model: hypr_transcribe_soniqo::SoniqoModel,
    source: hypr_transcribe_soniqo::TranscriptSource,
    start: f64,
    response_tx: &tokio::sync::mpsc::Sender<Result<StreamResponse, LocalSoniqoLiveError>>,
) -> Result<hypr_transcribe_soniqo::LiveTranscriptionSession, LocalSoniqoLiveError> {
    let joined = tokio::task::spawn_blocking(move || {
        let mut session = session;
        let result = session.finalize(source);
        (session, result)
    })
    .await
    .map_err(|error| LocalSoniqoLiveError::FinalizeJoin(error.to_string()))?;

    let (session, partials) = joined;
    let partials = partials.map_err(|error| LocalSoniqoLiveError::Finalize(error.to_string()))?;

    for partial in partials {
        let response = partial.into_stream_response(model, start, 0.05);
        if response_tx.send(Ok(response)).await.is_err() {
            break;
        }
    }

    Ok(session)
}

fn i16_bytes_to_f32(bytes: &Bytes) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / i16::MAX as f32)
        .collect()
}

fn suppress_echo_dominant_mic(mic: &mut [f32], speaker: &[f32]) -> bool {
    let Some(score) = best_echo_score(mic, speaker) else {
        return false;
    };

    if score.correlation < ECHO_GATE_MIN_CORRELATION
        || score.residual_ratio > ECHO_GATE_MAX_RESIDUAL_RATIO
        || score.mic_rms < ECHO_GATE_MIN_MIC_RMS
        || score.speaker_rms < ECHO_GATE_MIN_SPEAKER_RMS
    {
        return false;
    }

    mic.fill(0.0);
    true
}

#[derive(Debug, Clone, Copy)]
struct EchoScore {
    correlation: f32,
    residual_ratio: f32,
    mic_rms: f32,
    speaker_rms: f32,
}

fn best_echo_score(mic: &[f32], speaker: &[f32]) -> Option<EchoScore> {
    let len = mic.len().min(speaker.len());
    if len < ECHO_GATE_MIN_SAMPLES {
        return None;
    }

    let max_lag = ECHO_GATE_MAX_LAG_SAMPLES.min((len - ECHO_GATE_MIN_SAMPLES) as isize);
    let mut best = echo_score_at_lag(&mic[..len], &speaker[..len], 0);
    let mut lag = ECHO_GATE_LAG_STEP_SAMPLES;

    while lag <= max_lag {
        for lag in [-lag, lag] {
            if let Some(score) = echo_score_at_lag(&mic[..len], &speaker[..len], lag) {
                if best
                    .map(|current| score.correlation > current.correlation)
                    .unwrap_or(true)
                {
                    best = Some(score);
                }
            }
        }
        lag += ECHO_GATE_LAG_STEP_SAMPLES;
    }

    best
}

fn echo_score_at_lag(mic: &[f32], speaker: &[f32], lag: isize) -> Option<EchoScore> {
    let len = mic.len().min(speaker.len());
    let (mic_start, speaker_start) = if lag >= 0 {
        (lag as usize, 0)
    } else {
        (0, lag.unsigned_abs())
    };
    let overlap = len.saturating_sub(mic_start.max(speaker_start));
    if overlap < ECHO_GATE_MIN_SAMPLES {
        return None;
    }

    let mut mic_energy = 0.0;
    let mut speaker_energy = 0.0;
    let mut cross_energy = 0.0;
    for idx in 0..overlap {
        let mic_sample = mic[mic_start + idx];
        let speaker_sample = speaker[speaker_start + idx];
        mic_energy += mic_sample * mic_sample;
        speaker_energy += speaker_sample * speaker_sample;
        cross_energy += mic_sample * speaker_sample;
    }

    if mic_energy <= f32::EPSILON || speaker_energy <= f32::EPSILON {
        return None;
    }

    let overlap_f32 = overlap as f32;
    let mic_rms = (mic_energy / overlap_f32).sqrt();
    let speaker_rms = (speaker_energy / overlap_f32).sqrt();
    let correlation = cross_energy.abs() / (mic_energy * speaker_energy).sqrt().max(1e-6);
    let residual_energy =
        (mic_energy - (cross_energy * cross_energy / speaker_energy.max(1e-6))).max(0.0);
    let residual_ratio = residual_energy.sqrt() / mic_energy.sqrt().max(1e-6);

    Some(EchoScore {
        correlation,
        residual_ratio,
        mic_rms,
        speaker_rms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_signal(len: usize, seed: u32) -> Vec<f32> {
        let mut state = seed;
        (0..len)
            .map(|idx| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                let noise = (state as f32 / u32::MAX as f32) * 2.0 - 1.0;
                let pulse = if idx % 257 == 0 { 0.8 } else { 0.0 };
                0.45 * noise + pulse
            })
            .collect()
    }

    fn delayed(input: &[f32], delay_samples: usize, gain: f32) -> Vec<f32> {
        let mut out = vec![0.0; input.len()];
        for idx in delay_samples..input.len() {
            out[idx] = input[idx - delay_samples] * gain;
        }
        out
    }

    #[test]
    fn suppresses_mic_when_chunk_is_echo_dominant() {
        let speaker = test_signal(1920, 0x1234_5678);
        let mut mic = delayed(&speaker, 160, 0.35);

        assert!(suppress_echo_dominant_mic(&mut mic, &speaker));
        assert!(mic.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn suppresses_mic_when_aec_residual_has_longer_capture_lag() {
        let speaker = test_signal(2400, 0x1234_5678);
        let mut mic = delayed(&speaker, 1120, 0.25);

        assert!(suppress_echo_dominant_mic(&mut mic, &speaker));
        assert!(mic.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn suppresses_mic_when_minimum_chunk_matches_zero_lag() {
        let speaker = test_signal(ECHO_GATE_MIN_SAMPLES, 0x1234_5678);
        let mut mic: Vec<f32> = speaker.iter().map(|sample| sample * 0.35).collect();

        assert!(suppress_echo_dominant_mic(&mut mic, &speaker));
        assert!(mic.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn best_echo_score_considers_zero_lag_when_step_grid_skips_it() {
        let speaker = test_signal(1024, 0x1234_5678);
        let mic: Vec<f32> = speaker.iter().map(|sample| sample * 0.35).collect();

        let score = best_echo_score(&mic, &speaker).expect("echo score");

        assert!(score.correlation > 0.99);
    }

    #[test]
    fn keeps_mic_when_independent_speech_dominates() {
        let speaker = test_signal(1920, 0x1234_5678);
        let local = test_signal(1920, 0x8765_4321);
        let mut mic = delayed(&speaker, 160, 0.15);
        for (mic_sample, local_sample) in mic.iter_mut().zip(local) {
            *mic_sample += 0.45 * local_sample;
        }
        let original = mic.clone();

        assert!(!suppress_echo_dominant_mic(&mut mic, &speaker));
        assert_eq!(mic, original);
    }

    #[test]
    fn keeps_mic_when_speaker_reference_is_quiet() {
        let speaker = vec![0.001; 1920];
        let mut mic = test_signal(1920, 0x1234_5678);
        let original = mic.clone();

        assert!(!suppress_echo_dominant_mic(&mut mic, &speaker));
        assert_eq!(mic, original);
    }
}

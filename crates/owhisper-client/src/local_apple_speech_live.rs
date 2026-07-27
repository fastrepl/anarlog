use std::time::Duration;

use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use owhisper_interface::stream::StreamResponse;
use owhisper_interface::{ControlMessage, MixedMessage};
use tokio_stream::wrappers::ReceiverStream;

use crate::{FinalizeHandle, ListenClientDualInput, ListenClientInput};

const FLUSH_INTERVAL: Duration = Duration::from_millis(250);

pub type LocalAppleSpeechLiveStream =
    ReceiverStream<Result<StreamResponse, LocalAppleSpeechLiveError>>;

#[derive(Debug, thiserror::Error)]
pub enum LocalAppleSpeechLiveError {
    #[error("apple_speech_live_start_join_failed: {0}")]
    StartJoin(String),
    #[error("apple_speech_live_start_failed: {0}")]
    Start(String),
    #[error("apple_speech_live_append_join_failed: {0}")]
    AppendJoin(String),
    #[error("apple_speech_live_append_failed: {0}")]
    Append(String),
    #[error("apple_speech_live_finalize_join_failed: {0}")]
    FinalizeJoin(String),
    #[error("apple_speech_live_finalize_failed: {0}")]
    Finalize(String),
}

pub struct LocalAppleSpeechLiveClient {
    locale: String,
}

impl LocalAppleSpeechLiveClient {
    pub fn new(locale: impl Into<String>) -> Self {
        Self {
            locale: locale.into(),
        }
    }

    pub async fn from_realtime_audio_single(
        self,
        stream: impl Stream<Item = ListenClientInput> + Send + Unpin + 'static,
        source: hypr_transcribe_speechanalyzer::TranscriptSource,
    ) -> Result<(LocalAppleSpeechLiveStream, LocalAppleSpeechLiveHandle), LocalAppleSpeechLiveError>
    {
        let session = start_session(self.locale).await?;
        let (response_tx, response_rx) = tokio::sync::mpsc::channel(32);
        let (finalize_tx, finalize_rx) = tokio::sync::mpsc::channel(1);

        tokio::spawn(run_single(
            session,
            source,
            stream,
            response_tx,
            finalize_rx,
        ));

        Ok((
            ReceiverStream::new(response_rx),
            LocalAppleSpeechLiveHandle {
                finalize_tx,
                expected_finalize_count: 1,
            },
        ))
    }

    pub async fn from_realtime_audio_dual(
        self,
        stream: impl Stream<Item = ListenClientDualInput> + Send + Unpin + 'static,
    ) -> Result<(LocalAppleSpeechLiveStream, LocalAppleSpeechLiveHandle), LocalAppleSpeechLiveError>
    {
        let session = start_session(self.locale).await?;
        let (response_tx, response_rx) = tokio::sync::mpsc::channel(32);
        let (finalize_tx, finalize_rx) = tokio::sync::mpsc::channel(1);

        tokio::spawn(run_dual(session, stream, response_tx, finalize_rx));

        Ok((
            ReceiverStream::new(response_rx),
            LocalAppleSpeechLiveHandle {
                finalize_tx,
                expected_finalize_count: 2,
            },
        ))
    }
}

pub struct LocalAppleSpeechLiveHandle {
    finalize_tx: tokio::sync::mpsc::Sender<()>,
    expected_finalize_count: usize,
}

impl FinalizeHandle for LocalAppleSpeechLiveHandle {
    async fn finalize(&self) {
        let _ = self.finalize_tx.send(()).await;
    }

    fn expected_finalize_count(&self) -> usize {
        self.expected_finalize_count
    }
}

type Session = hypr_transcribe_speechanalyzer::LiveTranscriptionSession;
type Source = hypr_transcribe_speechanalyzer::TranscriptSource;
type ResponseSender = tokio::sync::mpsc::Sender<Result<StreamResponse, LocalAppleSpeechLiveError>>;

async fn run_single(
    session: Session,
    source: Source,
    mut stream: impl Stream<Item = ListenClientInput> + Send + Unpin + 'static,
    response_tx: ResponseSender,
    mut finalize_rx: tokio::sync::mpsc::Receiver<()>,
) {
    let mut session = session;
    let mut buffer = Vec::<f32>::new();
    let mut interval = tokio::time::interval(FLUSH_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = finalize_rx.recv() => {
                let result = finalize_single(session, source, &mut buffer, &response_tx).await;
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
                        let result = finalize_single(session, source, &mut buffer, &response_tx).await;
                        stop_after_finalize(result).await;
                        return;
                    }
                    MixedMessage::Control(ControlMessage::KeepAlive) => {}
                }
            }
            _ = interval.tick() => {
                match flush_buffer(session, source, &mut buffer, &response_tx).await {
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
    session: Session,
    mut stream: impl Stream<Item = ListenClientDualInput> + Send + Unpin + 'static,
    response_tx: ResponseSender,
    mut finalize_rx: tokio::sync::mpsc::Receiver<()>,
) {
    let mut session = session;
    let mut mic_buffer = Vec::<f32>::new();
    let mut spk_buffer = Vec::<f32>::new();
    let mut interval = tokio::time::interval(FLUSH_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = finalize_rx.recv() => {
                let result = finalize_dual(session, &mut mic_buffer, &mut spk_buffer, &response_tx).await;
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
                        mic_buffer.extend(i16_bytes_to_f32(&mic));
                        spk_buffer.extend(i16_bytes_to_f32(&spk));
                    }
                    MixedMessage::Control(ControlMessage::CloseStream | ControlMessage::Finalize) => {
                        let result = finalize_dual(session, &mut mic_buffer, &mut spk_buffer, &response_tx).await;
                        stop_after_finalize(result).await;
                        return;
                    }
                    MixedMessage::Control(ControlMessage::KeepAlive) => {}
                }
            }
            _ = interval.tick() => {
                match flush_buffer(session, Source::Microphone, &mut mic_buffer, &response_tx).await {
                    Ok(next_session) => session = next_session,
                    Err(error) => {
                        let _ = response_tx.send(Err(error)).await;
                        return;
                    }
                }

                match flush_buffer(session, Source::System, &mut spk_buffer, &response_tx).await {
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
    session: Session,
    source: Source,
    buffer: &mut Vec<f32>,
    response_tx: &ResponseSender,
) -> Result<Session, LocalAppleSpeechLiveError> {
    let session = flush_buffer(session, source, buffer, response_tx).await?;
    finalize_source(session, source, response_tx).await
}

async fn finalize_dual(
    session: Session,
    mic_buffer: &mut Vec<f32>,
    spk_buffer: &mut Vec<f32>,
    response_tx: &ResponseSender,
) -> Result<Session, LocalAppleSpeechLiveError> {
    let session = flush_buffer(session, Source::Microphone, mic_buffer, response_tx).await?;
    let session = flush_buffer(session, Source::System, spk_buffer, response_tx).await?;
    let session = finalize_source(session, Source::Microphone, response_tx).await?;
    finalize_source(session, Source::System, response_tx).await
}

async fn stop_after_finalize(result: Result<Session, LocalAppleSpeechLiveError>) {
    match result {
        Ok(session) => stop_session(session).await,
        Err(error) => tracing::warn!(error.message = %error, "apple_speech_live_finalize_failed"),
    }
}

async fn start_session(locale: String) -> Result<Session, LocalAppleSpeechLiveError> {
    tokio::task::spawn_blocking(move || Session::start(&locale))
        .await
        .map_err(|error| LocalAppleSpeechLiveError::StartJoin(error.to_string()))?
        .map_err(|error| LocalAppleSpeechLiveError::Start(error.to_string()))
}

async fn stop_session(session: Session) {
    let _ = tokio::task::spawn_blocking(move || session.stop()).await;
}

async fn flush_buffer(
    session: Session,
    source: Source,
    buffer: &mut Vec<f32>,
    response_tx: &ResponseSender,
) -> Result<Session, LocalAppleSpeechLiveError> {
    let samples = std::mem::take(buffer);
    let joined = tokio::task::spawn_blocking(move || {
        let mut session = session;
        let result = session.append(source, &samples);
        (session, result)
    })
    .await
    .map_err(|error| LocalAppleSpeechLiveError::AppendJoin(error.to_string()))?;

    let (session, partials) = joined;
    let partials =
        partials.map_err(|error| LocalAppleSpeechLiveError::Append(error.to_string()))?;

    send_partials(partials, response_tx).await;
    Ok(session)
}

async fn finalize_source(
    session: Session,
    source: Source,
    response_tx: &ResponseSender,
) -> Result<Session, LocalAppleSpeechLiveError> {
    let joined = tokio::task::spawn_blocking(move || {
        let mut session = session;
        let result = session.finalize(source);
        (session, result)
    })
    .await
    .map_err(|error| LocalAppleSpeechLiveError::FinalizeJoin(error.to_string()))?;

    let (session, partials) = joined;
    let partials =
        partials.map_err(|error| LocalAppleSpeechLiveError::Finalize(error.to_string()))?;

    send_partials(partials, response_tx).await;
    Ok(session)
}

/// Results carry their own timeline from the analyzer, so no cursor bookkeeping is needed.
async fn send_partials(
    partials: Vec<hypr_transcribe_speechanalyzer::LivePartial>,
    response_tx: &ResponseSender,
) {
    for partial in partials {
        if response_tx
            .send(Ok(partial.into_stream_response()))
            .await
            .is_err()
        {
            break;
        }
    }
}

fn i16_bytes_to_f32(bytes: &Bytes) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / i16::MAX as f32)
        .collect()
}

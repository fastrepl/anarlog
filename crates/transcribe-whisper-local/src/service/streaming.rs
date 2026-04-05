use std::{
    collections::VecDeque,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use axum::{
    body::Body,
    extract::{FromRequestParts, ws::WebSocketUpgrade},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, Stream, StreamExt, stream::poll_fn};
use hypr_audio_chunking::{SpeechChunkExt, SpeechChunkingConfig};
use hypr_audio_interface::AsyncSource;
use hypr_model_manager::{ModelManager, ModelManagerBuilder};
use hypr_transcribe_core::TARGET_SAMPLE_RATE;
use hypr_ws_utils::ConnectionManager;
use owhisper_interface::stream::StreamResponse;
use owhisper_interface::{ControlMessage, ListenParams};
use tokio::sync::mpsc;
use tower::Service;

use super::batch;
use super::message::{AudioExtract, IncomingMessage, process_incoming_message};
use super::response::{
    TranscriptKind, build_transcript_response, format_timestamp_now, send_ws, send_ws_best_effort,
};
use super::{
    build_metadata, build_model_with_languages, parse_listen_params, redemption_time,
    transcribe_chunk,
};

pub const LISTEN_PATH: &str = "/v1/listen";
pub const HEALTH_PATH: &str = "/health";

#[derive(Clone)]
pub struct TranscribeService {
    model_path: PathBuf,
    manager: ModelManager<hypr_whisper_local::LoadedWhisper>,
    connection_manager: ConnectionManager,
}

impl TranscribeService {
    pub fn builder() -> TranscribeServiceBuilder {
        TranscribeServiceBuilder::default()
    }

    pub fn into_router<F, Fut>(self, on_error: F) -> axum::Router
    where
        F: FnOnce(String) -> Fut + Clone + Send + Sync + 'static,
        Fut: std::future::Future<Output = (StatusCode, String)> + Send,
    {
        let service = axum::error_handling::HandleError::new(self, on_error);
        axum::Router::new()
            .route(HEALTH_PATH, axum::routing::get(|| async { "ok" }))
            .route_service(LISTEN_PATH, service)
    }
}

#[derive(Default)]
pub struct TranscribeServiceBuilder {
    model_path: Option<PathBuf>,
    connection_manager: Option<ConnectionManager>,
}

impl TranscribeServiceBuilder {
    pub fn model_path(mut self, model_path: PathBuf) -> Self {
        self.model_path = Some(model_path);
        self
    }

    pub fn build(self) -> TranscribeService {
        let model_path = self
            .model_path
            .expect("TranscribeServiceBuilder requires model_path");
        let manager = ModelManagerBuilder::default()
            .register("default", &model_path)
            .default_model("default")
            .build();

        let warmup_manager = manager.clone();
        tokio::spawn(async move {
            match warmup_manager.get(None).await {
                Ok(_) => tracing::info!("whisper_local_model_warmup_completed"),
                Err(error) => tracing::warn!(error = %error, "whisper_local_model_warmup_failed"),
            }
        });

        TranscribeService {
            model_path,
            manager,
            connection_manager: self.connection_manager.unwrap_or_default(),
        }
    }
}

impl Service<Request<Body>> for TranscribeService {
    type Response = Response;
    type Error = String;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let model_path = self.model_path.clone();
        let manager = self.manager.clone();
        let connection_manager = self.connection_manager.clone();

        Box::pin(async move {
            let is_ws = req
                .headers()
                .get("upgrade")
                .and_then(|value| value.to_str().ok())
                .map(|value| value.eq_ignore_ascii_case("websocket"))
                .unwrap_or(false);

            let params = match parse_listen_params(req.uri().query().unwrap_or("")) {
                Ok(params) => params,
                Err(error) => {
                    return Ok((StatusCode::BAD_REQUEST, error.to_string()).into_response());
                }
            };

            if is_ws {
                let model = match manager.get(None).await {
                    Ok(model) => model,
                    Err(error) => {
                        tracing::error!(error = %error, "failed_to_load_model");
                        return Ok((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("failed to load model: {error}"),
                        )
                            .into_response());
                    }
                };

                let metadata = build_metadata(&model_path);
                let (mut parts, _body) = req.into_parts();
                let ws_upgrade = match WebSocketUpgrade::from_request_parts(&mut parts, &()).await {
                    Ok(ws) => ws,
                    Err(error) => {
                        return Ok((StatusCode::BAD_REQUEST, error.to_string()).into_response());
                    }
                };

                let guard = connection_manager.acquire_connection();
                Ok(ws_upgrade
                    .on_upgrade(move |socket| async move {
                        handle_websocket(socket, params, metadata, guard, model, manager).await;
                    })
                    .into_response())
            } else {
                let content_type = req
                    .headers()
                    .get("content-type")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let accept = req
                    .headers()
                    .get("accept")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                let body = match axum::body::to_bytes(req.into_body(), 100 * 1024 * 1024).await {
                    Ok(body) => body,
                    Err(error) => {
                        return Ok((StatusCode::BAD_REQUEST, error.to_string()).into_response());
                    }
                };

                if body.is_empty() {
                    return Ok((StatusCode::BAD_REQUEST, "request body is empty").into_response());
                }

                if accept.contains("text/event-stream") {
                    Ok(
                        batch::handle_batch_sse(
                            body,
                            &content_type,
                            &params,
                            &manager,
                            &model_path,
                        )
                        .await,
                    )
                } else {
                    Ok(
                        batch::handle_batch(body, &content_type, &params, &manager, &model_path)
                            .await,
                    )
                }
            }
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StopReason {
    End,
    Finalize,
}

async fn handle_websocket(
    socket: axum::extract::ws::WebSocket,
    params: ListenParams,
    metadata: owhisper_interface::stream::Metadata,
    guard: hypr_ws_utils::ConnectionGuard,
    model: Arc<hypr_whisper_local::LoadedWhisper>,
    manager: ModelManager<hypr_whisper_local::LoadedWhisper>,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let total_channels = (params.channels as usize).max(1);
    let redemption_time = redemption_time(&params);
    let languages: Vec<hypr_whisper::Language> = params
        .languages
        .iter()
        .filter_map(|lang| lang.clone().try_into().ok())
        .collect();
    match build_transcription_streams(total_channels, model.as_ref(), &languages, redemption_time) {
        Ok((audio_txs, mut stream)) => {
            let mut audio_txs = audio_txs;
            let mut stop_reason = None;
            let mut receiving_input = true;
            let mut channel_audio_durations = vec![0.0_f64; total_channels];
            let mut stream_closed = false;

            while !stream_closed {
                tokio::select! {
                    _ = guard.cancelled() => {
                        tracing::info!("websocket_cancelled_by_new_connection");
                        break;
                    }
                    item = stream.next() => {
                        match item {
                            Some(Ok((channel_idx, segment))) => {
                                let channel_index = vec![channel_idx as i32, total_channels as i32];
                                let channel = vec![channel_idx as u8];
                                let transcript_kind = if stop_reason == Some(StopReason::Finalize) {
                                    TranscriptKind::Finalized
                                } else {
                                    TranscriptKind::Confirmed
                                };

                                if !send_ws(&mut ws_sender, &StreamResponse::SpeechStartedResponse {
                                    channel: channel.clone(),
                                    timestamp: segment.start,
                                }).await {
                                    break;
                                }

                                if !send_ws(
                                    &mut ws_sender,
                                    &build_transcript_response(&segment, transcript_kind, &metadata, &channel_index),
                                ).await {
                                    break;
                                }

                                if !send_ws(&mut ws_sender, &StreamResponse::UtteranceEndResponse {
                                    channel,
                                    last_word_end: segment.start + segment.duration,
                                }).await {
                                    break;
                                }
                            }
                            Some(Err(error)) => {
                                send_ws_best_effort(
                                    &mut ws_sender,
                                    &StreamResponse::ErrorResponse {
                                        error_code: None,
                                        error_message: error.to_string(),
                                        provider: "whisper-local".to_string(),
                                    },
                                )
                                .await;
                                break;
                            }
                            None => {
                                stream_closed = true;
                            }
                        }
                    }
                    message = ws_receiver.next(), if receiving_input => {
                        manager.keep_alive().await;

                        let Some(message) = message else {
                            receiving_input = false;
                            stop_reason.get_or_insert(StopReason::End);
                            audio_txs.clear();
                            continue;
                        };

                        let message = match message {
                            Ok(message) => message,
                            Err(error) => {
                                send_ws_best_effort(
                                    &mut ws_sender,
                                    &StreamResponse::ErrorResponse {
                                        error_code: None,
                                        error_message: format!("websocket receive error: {error}"),
                                        provider: "whisper-local".to_string(),
                                    },
                                )
                                .await;
                                break;
                            }
                        };

                        match process_incoming_message(&message, params.channels.max(1)) {
                            Ok(IncomingMessage::Audio(AudioExtract::Mono(samples))) => {
                                if samples.is_empty() {
                                    continue;
                                }
                                channel_audio_durations[0] += samples.len() as f64 / TARGET_SAMPLE_RATE as f64;
                                if audio_txs[0].send(samples).await.is_err() {
                                    send_ws_best_effort(
                                        &mut ws_sender,
                                        &StreamResponse::ErrorResponse {
                                            error_code: None,
                                            error_message: "audio pipeline closed unexpectedly".to_string(),
                                            provider: "whisper-local".to_string(),
                                        },
                                    )
                                    .await;
                                    break;
                                }
                            }
                            Ok(IncomingMessage::Audio(AudioExtract::Dual { ch0, ch1 })) => {
                                if total_channels >= 2 {
                                    channel_audio_durations[0] += ch0.len() as f64 / TARGET_SAMPLE_RATE as f64;
                                    channel_audio_durations[1] += ch1.len() as f64 / TARGET_SAMPLE_RATE as f64;
                                    if audio_txs[0].send(ch0).await.is_err() || audio_txs[1].send(ch1).await.is_err() {
                                        send_ws_best_effort(
                                            &mut ws_sender,
                                            &StreamResponse::ErrorResponse {
                                                error_code: None,
                                                error_message: "audio pipeline closed unexpectedly".to_string(),
                                                provider: "whisper-local".to_string(),
                                            },
                                        )
                                        .await;
                                        break;
                                    }
                                } else {
                                    let mixed = hypr_audio_utils::mix_audio_f32(&ch0, &ch1);
                                    channel_audio_durations[0] += mixed.len() as f64 / TARGET_SAMPLE_RATE as f64;
                                    if !mixed.is_empty() && audio_txs[0].send(mixed).await.is_err() {
                                        send_ws_best_effort(
                                            &mut ws_sender,
                                            &StreamResponse::ErrorResponse {
                                                error_code: None,
                                                error_message: "audio pipeline closed unexpectedly".to_string(),
                                                provider: "whisper-local".to_string(),
                                            },
                                        )
                                        .await;
                                        break;
                                    }
                                }
                            }
                            Ok(IncomingMessage::Audio(AudioExtract::End)) => {
                                receiving_input = false;
                                stop_reason.get_or_insert(StopReason::End);
                                audio_txs.clear();
                            }
                            Ok(IncomingMessage::Audio(AudioExtract::Empty)) => {}
                            Ok(IncomingMessage::Control(ControlMessage::KeepAlive)) => {}
                            Ok(IncomingMessage::Control(ControlMessage::Finalize)) => {
                                receiving_input = false;
                                stop_reason = Some(StopReason::Finalize);
                                audio_txs.clear();
                            }
                            Ok(IncomingMessage::Control(ControlMessage::CloseStream)) => {
                                receiving_input = false;
                                stop_reason.get_or_insert(StopReason::End);
                                audio_txs.clear();
                            }
                            Err(error) => {
                                send_ws_best_effort(
                                    &mut ws_sender,
                                    &StreamResponse::ErrorResponse {
                                        error_code: None,
                                        error_message: error.to_string(),
                                        provider: "whisper-local".to_string(),
                                    },
                                )
                                .await;
                                break;
                            }
                        }
                    }
                }
            }

            if stream_closed {
                let total_duration = channel_audio_durations.into_iter().fold(0.0_f64, f64::max);
                send_ws_best_effort(
                    &mut ws_sender,
                    &StreamResponse::TerminalResponse {
                        request_id: metadata.request_id.clone(),
                        created: format_timestamp_now(),
                        duration: total_duration,
                        channels: total_channels as u32,
                    },
                )
                .await;
            }

            let _ = ws_sender.close().await;
        }
        Err(error) => {
            send_ws_best_effort(
                &mut ws_sender,
                &StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: error.to_string(),
                    provider: "whisper-local".to_string(),
                },
            )
            .await;
            let _ = ws_sender.close().await;
        }
    }
}

type TranscriptionStream =
    Pin<Box<dyn Stream<Item = Result<(usize, crate::service::Segment), crate::Error>> + Send>>;

#[allow(clippy::type_complexity)]
fn build_transcription_streams(
    total_channels: usize,
    loaded_model: &hypr_whisper_local::LoadedWhisper,
    languages: &[hypr_whisper::Language],
    redemption_time: std::time::Duration,
) -> Result<
    (
        Vec<mpsc::Sender<Vec<f32>>>,
        futures_util::stream::SelectAll<TranscriptionStream>,
    ),
    crate::Error,
> {
    let mut audio_txs = Vec::with_capacity(total_channels);
    let mut streams = futures_util::stream::SelectAll::new();

    for channel_idx in 0..total_channels {
        let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(8);
        audio_txs.push(audio_tx);

        let model = build_model_with_languages(loaded_model, languages.to_vec())?;
        let chunk_stream = ChannelAudioSource::new(audio_rx)
            .speech_chunks(SpeechChunkingConfig::speech(redemption_time));
        let stream: TranscriptionStream = Box::pin(TranscribeChannelStream::new(
            channel_idx,
            chunk_stream,
            model,
        ));
        streams.push(stream);
    }

    Ok((audio_txs, streams))
}

struct ChannelAudioSource {
    receiver: mpsc::Receiver<Vec<f32>>,
    buffered: VecDeque<f32>,
}

impl ChannelAudioSource {
    fn new(receiver: mpsc::Receiver<Vec<f32>>) -> Self {
        Self {
            receiver,
            buffered: VecDeque::new(),
        }
    }
}

impl AsyncSource for ChannelAudioSource {
    fn as_stream(&mut self) -> impl Stream<Item = f32> + '_ {
        poll_fn(move |cx| {
            loop {
                if let Some(sample) = self.buffered.pop_front() {
                    return Poll::Ready(Some(sample));
                }

                match self.receiver.poll_recv(cx) {
                    Poll::Ready(Some(chunk)) => {
                        self.buffered.extend(chunk);
                        continue;
                    }
                    Poll::Ready(None) => return Poll::Ready(None),
                    Poll::Pending => return Poll::Pending,
                }
            }
        })
    }

    fn sample_rate(&self) -> u32 {
        TARGET_SAMPLE_RATE
    }
}

struct TranscribeChannelStream<S> {
    channel_idx: usize,
    chunk_stream: S,
    model: hypr_whisper_local::Whisper,
    pending: VecDeque<crate::service::Segment>,
}

impl<S> TranscribeChannelStream<S> {
    fn new(channel_idx: usize, chunk_stream: S, model: hypr_whisper_local::Whisper) -> Self {
        Self {
            channel_idx,
            chunk_stream,
            model,
            pending: VecDeque::new(),
        }
    }
}

impl<S> Stream for TranscribeChannelStream<S>
where
    S: Stream<Item = Result<hypr_audio_chunking::AudioChunk, hypr_audio_chunking::Error>> + Unpin,
{
    type Item = Result<(usize, crate::service::Segment), crate::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(segment) = self.pending.pop_front() {
            return Poll::Ready(Some(Ok((self.channel_idx, segment))));
        }

        loop {
            match Pin::new(&mut self.chunk_stream).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    let start_sec = chunk.sample_start as f64 / TARGET_SAMPLE_RATE as f64;
                    match transcribe_chunk(&mut self.model, &chunk.samples, start_sec) {
                        Ok(segments) => {
                            self.pending.extend(segments);
                            if let Some(segment) = self.pending.pop_front() {
                                return Poll::Ready(Some(Ok((self.channel_idx, segment))));
                            }
                        }
                        Err(error) => return Poll::Ready(Some(Err(error))),
                    }
                }
                Poll::Ready(Some(Err(error))) => {
                    return Poll::Ready(Some(Err(error.into())));
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::build_metadata;

    #[test]
    fn health_and_listen_paths_are_stable() {
        assert_eq!(HEALTH_PATH, "/health");
        assert_eq!(LISTEN_PATH, "/v1/listen");
    }

    #[test]
    fn metadata_uses_model_info() {
        let metadata = build_metadata(std::path::Path::new("/tmp/model.bin"));
        assert_eq!(metadata.model_info.arch, "whisper-local");
    }
}

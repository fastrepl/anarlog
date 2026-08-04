mod adapters;
mod stream;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use bytes::Bytes;
use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef, RpcReplyPort, SupervisionEvent};
use tokio::time::error::Elapsed;
use tracing::Instrument;

use owhisper_interface::stream::StreamResponse;
use owhisper_interface::{ControlMessage, MixedMessage};

use super::session::session_span;
use crate::{
    DegradedError, ListenerRuntime, LiveTranscriptEngine, SessionDataEvent, SessionErrorEvent,
    SessionProgressEvent,
};

use adapters::spawn_rx_task;

pub(super) const LISTEN_STREAM_TIMEOUT: Duration = Duration::from_secs(15 * 60);
pub(super) const FINALIZE_STREAM_TIMEOUT: Duration = Duration::from_secs(5);
const STREAM_TASK_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(7);
pub(super) const DEVICE_FINGERPRINT_HEADER: &str = "x-device-fingerprint";

pub enum ListenerMsg {
    AudioSingle(Bytes, RpcReplyPort<ListenerAudioResult>),
    AudioDual(Bytes, Bytes, RpcReplyPort<ListenerAudioResult>),
    UpdateConfig(ListenerConfigUpdate),
    StreamResponse(StreamResponse, RpcReplyPort<()>),
    StreamError(String),
    StreamEnded,
    StreamTimeout(Elapsed),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenerAudioResult {
    Accepted,
    Backpressured,
    Closed,
    ModeMismatch,
}

#[derive(Clone)]
pub struct ListenerConfigUpdate {
    pub languages: Vec<anlg_language::Language>,
    pub participant_human_ids: Vec<String>,
    pub self_human_id: Option<String>,
}

#[derive(Clone)]
pub struct ListenerArgs {
    pub runtime: Arc<dyn ListenerRuntime>,
    pub languages: Vec<anlg_language::Language>,
    pub onboarding: bool,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub keywords: Vec<String>,
    pub transcription_mode: crate::TranscriptionMode,
    pub mode: crate::actors::ChannelMode,
    pub session_started_at: Instant,
    pub session_started_at_unix: SystemTime,
    pub stream_offset_secs: Option<f64>,
    pub session_id: String,
    pub participant_human_ids: Vec<String>,
    pub self_human_id: Option<String>,
}

pub struct ListenerState {
    pub args: ListenerArgs,
    transcript: LiveTranscriptEngine,
    tx: ChannelSender,
    rx_task: tokio::task::JoinHandle<Vec<StreamResponse>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

pub(super) enum ChannelSender {
    Single(tokio::sync::mpsc::Sender<MixedMessage<Bytes, ControlMessage>>),
    Dual(tokio::sync::mpsc::Sender<MixedMessage<(Bytes, Bytes), ControlMessage>>),
}

pub struct ListenerActor;

impl ListenerActor {
    pub fn name() -> ActorName {
        "listener_actor".into()
    }
}

#[derive(Debug)]
pub(super) struct ListenerInitError {
    pub(super) message: String,
    pub(super) degraded: Option<DegradedError>,
}

impl std::fmt::Display for ListenerInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ListenerInitError {}

pub(super) fn actor_error(msg: impl Into<String>) -> ActorProcessingErr {
    Box::new(ListenerInitError {
        message: msg.into(),
        degraded: None,
    })
}

pub(super) fn actor_error_with_degraded(
    msg: impl Into<String>,
    degraded: DegradedError,
) -> ActorProcessingErr {
    Box::new(ListenerInitError {
        message: msg.into(),
        degraded: Some(degraded),
    })
}

#[ractor::async_trait]
impl Actor for ListenerActor {
    type Msg = ListenerMsg;
    type State = ListenerState;
    type Arguments = ListenerArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let session_id = args.session_id.clone();
        let span = session_span(&session_id);

        async {
            args.runtime
                .emit_progress(SessionProgressEvent::Connecting {
                    session_id: session_id.clone(),
                });

            let (tx, rx_task, shutdown_tx, adapter_name) =
                spawn_rx_task(args.clone(), myself).await?;

            args.runtime.emit_progress(SessionProgressEvent::Connected {
                session_id: session_id.clone(),
                adapter: adapter_name.clone(),
            });

            let transcript = LiveTranscriptEngine::new(
                &adapter_name,
                &args.participant_human_ids,
                args.self_human_id.as_deref(),
            );

            let state = ListenerState {
                args,
                transcript,
                tx,
                rx_task,
                shutdown_tx: Some(shutdown_tx),
            };

            Ok(state)
        }
        .instrument(span)
        .await
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let final_responses = stop_stream_task(
            &mut state.shutdown_tx,
            &mut state.rx_task,
            STREAM_TASK_SHUTDOWN_TIMEOUT,
        )
        .await;

        for response in final_responses {
            let _ = process_stream_response(state, response);
        }

        if let Some(update) = state.transcript.flush() {
            if !update.transcript_delta.is_empty() {
                state
                    .args
                    .runtime
                    .emit_data(SessionDataEvent::TranscriptDelta {
                        session_id: state.args.session_id.clone(),
                        delta: Box::new(update.transcript_delta),
                    });
            }
            if let Some(segment_delta) = update.segment_delta {
                state
                    .args
                    .runtime
                    .emit_data(SessionDataEvent::TranscriptSegmentDelta {
                        session_id: state.args.session_id.clone(),
                        delta: Box::new(segment_delta),
                    });
            }
        }

        Ok(())
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let span = session_span(&state.args.session_id);
        let _guard = span.enter();

        match message {
            ListenerMsg::AudioSingle(audio, reply) => {
                let result = match &state.tx {
                    ChannelSender::Single(tx) => match tx.try_send(MixedMessage::Audio(audio)) {
                        Ok(()) => ListenerAudioResult::Accepted,
                        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                            ListenerAudioResult::Backpressured
                        }
                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                            ListenerAudioResult::Closed
                        }
                    },
                    ChannelSender::Dual(_) => ListenerAudioResult::ModeMismatch,
                };
                let _ = reply.send(result);
                if matches!(
                    result,
                    ListenerAudioResult::Closed | ListenerAudioResult::ModeMismatch
                ) {
                    stop_with_degraded_error(
                        &myself,
                        DegradedError::StreamError {
                            message: "listener audio channel unavailable".to_string(),
                        },
                    );
                }
            }

            ListenerMsg::AudioDual(mic, spk, reply) => {
                let result = match &state.tx {
                    ChannelSender::Dual(tx) => match tx.try_send(MixedMessage::Audio((mic, spk))) {
                        Ok(()) => ListenerAudioResult::Accepted,
                        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                            ListenerAudioResult::Backpressured
                        }
                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                            ListenerAudioResult::Closed
                        }
                    },
                    ChannelSender::Single(_) => ListenerAudioResult::ModeMismatch,
                };
                let _ = reply.send(result);
                if matches!(
                    result,
                    ListenerAudioResult::Closed | ListenerAudioResult::ModeMismatch
                ) {
                    stop_with_degraded_error(
                        &myself,
                        DegradedError::StreamError {
                            message: "listener audio channel unavailable".to_string(),
                        },
                    );
                }
            }

            ListenerMsg::UpdateConfig(update) => {
                state.args.languages = update.languages;
                state.args.participant_human_ids = update.participant_human_ids;
                state.args.self_human_id = update.self_human_id;
                state.transcript.update_participants(
                    &state.args.participant_human_ids,
                    state.args.self_human_id.as_deref(),
                );
            }

            ListenerMsg::StreamResponse(response, reply) => {
                let degraded = process_stream_response(state, response);
                let _ = reply.send(());
                if let Some(degraded) = degraded {
                    stop_with_degraded_error(&myself, degraded);
                }
            }

            ListenerMsg::StreamError(error) => {
                tracing::info!("listen_stream_error: {}", error);
                stop_with_degraded_error(&myself, DegradedError::StreamError { message: error });
            }

            ListenerMsg::StreamEnded => {
                tracing::info!("listen_stream_ended");
                stop_with_degraded_error(
                    &myself,
                    DegradedError::UpstreamUnavailable {
                        message: "stream ended".to_string(),
                    },
                );
            }

            ListenerMsg::StreamTimeout(elapsed) => {
                tracing::info!("listen_stream_timeout: {}", elapsed);
                stop_with_degraded_error(&myself, DegradedError::ConnectionTimeout);
            }
        }
        Ok(())
    }

    async fn handle_supervisor_evt(
        &self,
        myself: ActorRef<Self::Msg>,
        message: SupervisionEvent,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let span = session_span(&state.args.session_id);
        let _guard = span.enter();
        tracing::info!("supervisor_event: {:?}", message);

        match message {
            SupervisionEvent::ActorStarted(_) | SupervisionEvent::ProcessGroupChanged(_) => {}
            SupervisionEvent::ActorTerminated(_, _, _) => {}
            SupervisionEvent::ActorFailed(_cell, _) => {
                myself.stop(None);
            }
        }
        Ok(())
    }
}

async fn stop_stream_task(
    shutdown_tx: &mut Option<tokio::sync::oneshot::Sender<()>>,
    rx_task: &mut tokio::task::JoinHandle<Vec<StreamResponse>>,
    timeout: Duration,
) -> Vec<StreamResponse> {
    let Some(shutdown_tx) = shutdown_tx.take() else {
        return Vec::new();
    };

    let _ = shutdown_tx.send(());
    match tokio::time::timeout(timeout, &mut *rx_task).await {
        Ok(Ok(responses)) => responses,
        Ok(Err(error)) => {
            tracing::warn!(error.message = ?error, "listener_stream_task_join_failed");
            Vec::new()
        }
        Err(_) => {
            tracing::warn!("listener_stream_task_shutdown_timed_out");
            rx_task.abort();
            Vec::new()
        }
    }
}

fn process_stream_response(
    state: &mut ListenerState,
    mut response: StreamResponse,
) -> Option<DegradedError> {
    if let StreamResponse::ErrorResponse {
        error_code,
        error_message,
        provider,
    } = &response
    {
        tracing::error!(
            ?error_code,
            %error_message,
            %provider,
            "stream_provider_error"
        );
        state
            .args
            .runtime
            .emit_error(SessionErrorEvent::ConnectionError {
                session_id: state.args.session_id.clone(),
                error: format!(
                    "[{}] {} (code: {})",
                    provider,
                    error_message,
                    error_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "none".to_string())
                ),
            });
        return Some(match *error_code {
            Some(401) | Some(403) => DegradedError::AuthenticationFailed {
                provider: provider.clone(),
            },
            _ => DegradedError::StreamError {
                message: format!("{}: {}", provider, error_message),
            },
        });
    }

    match state.args.mode {
        crate::actors::ChannelMode::MicOnly => response.remap_channel_index(0, 2),
        crate::actors::ChannelMode::SpeakerOnly => response.remap_channel_index(1, 2),
        crate::actors::ChannelMode::MicAndSpeaker => {}
    }

    if let Some(update) = state.transcript.process(&response) {
        state
            .args
            .runtime
            .emit_data(SessionDataEvent::TranscriptDelta {
                session_id: state.args.session_id.clone(),
                delta: Box::new(update.transcript_delta),
            });
        if let Some(segment_delta) = update.segment_delta {
            state
                .args
                .runtime
                .emit_data(SessionDataEvent::TranscriptSegmentDelta {
                    session_id: state.args.session_id.clone(),
                    delta: Box::new(segment_delta),
                });
        }
    }

    None
}

fn stop_with_degraded_error(myself: &ActorRef<ListenerMsg>, error: DegradedError) {
    let reason = serde_json::to_string(&error).ok();
    myself.stop(reason);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stream_task_shutdown_aborts_a_hung_task_after_the_deadline() {
        let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
        let mut shutdown_tx = Some(shutdown_tx);
        let mut task = tokio::spawn(std::future::pending::<Vec<StreamResponse>>());

        let responses =
            stop_stream_task(&mut shutdown_tx, &mut task, Duration::from_millis(20)).await;

        assert!(responses.is_empty());
        assert!(shutdown_tx.is_none());
        let join_result = tokio::time::timeout(Duration::from_secs(1), &mut task)
            .await
            .expect("aborted stream task should stop promptly");
        assert!(join_result.unwrap_err().is_cancelled());
    }
}

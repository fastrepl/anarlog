mod adapters;
mod stream;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use bytes::Bytes;
use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef, SupervisionEvent};
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
pub(super) const FINALIZE_STREAM_TIMEOUT: Duration = Duration::from_secs(2);
pub(super) const DEVICE_FINGERPRINT_HEADER: &str = "x-device-fingerprint";

pub enum ListenerMsg {
    AudioSingle(Bytes),
    AudioDual(Bytes, Bytes),
    StreamResponse(StreamResponse),
    StreamError(String),
    StreamEnded,
    StreamTimeout(Elapsed),
}

#[derive(Clone)]
pub struct ListenerArgs {
    pub runtime: Arc<dyn ListenerRuntime>,
    pub languages: Vec<hypr_language::Language>,
    pub onboarding: bool,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub keywords: Vec<String>,
    pub mode: crate::actors::ChannelMode,
    pub session_started_at: Instant,
    pub session_started_at_unix: SystemTime,
    pub session_id: String,
    pub participant_human_ids: Vec<String>,
    pub self_human_id: Option<String>,
}

pub struct ListenerState {
    pub args: ListenerArgs,
    transcript: LiveTranscriptEngine,
    tx: ChannelSender,
    rx_task: tokio::task::JoinHandle<()>,
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
pub(super) struct ListenerInitError(pub(super) String);

impl std::fmt::Display for ListenerInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ListenerInitError {}

pub(super) fn actor_error(msg: impl Into<String>) -> ActorProcessingErr {
    Box::new(ListenerInitError(msg.into()))
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
        if let Some(shutdown_tx) = state.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
            let _ = (&mut state.rx_task).await;
        }

        if let Some(update) = state.transcript.flush() {
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
            ListenerMsg::AudioSingle(audio) => {
                if let ChannelSender::Single(tx) = &state.tx {
                    let _ = tx.try_send(MixedMessage::Audio(audio));
                }
            }

            ListenerMsg::AudioDual(mic, spk) => {
                if let ChannelSender::Dual(tx) = &state.tx {
                    let _ = tx.try_send(MixedMessage::Audio((mic, spk)));
                }
            }

            ListenerMsg::StreamResponse(mut response) => {
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
                                    .map(|c| c.to_string())
                                    .unwrap_or_else(|| "none".to_string())
                            ),
                        });
                    let degraded = match *error_code {
                        Some(401) | Some(403) => DegradedError::AuthenticationFailed {
                            provider: provider.clone(),
                        },
                        _ => DegradedError::StreamError {
                            message: format!("{}: {}", provider, error_message),
                        },
                    };
                    stop_with_degraded_error(&myself, degraded);
                    return Ok(());
                }

                match state.args.mode {
                    crate::actors::ChannelMode::MicOnly => {
                        response.remap_channel_index(0, 2);
                    }
                    crate::actors::ChannelMode::SpeakerOnly => {
                        response.remap_channel_index(1, 2);
                    }
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

fn stop_with_degraded_error(myself: &ActorRef<ListenerMsg>, error: DegradedError) {
    let reason = serde_json::to_string(&error).ok();
    myself.stop(reason);
}

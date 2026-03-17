use std::sync::{Arc, Mutex};
use std::time::Duration;

use owhisper_client::StreamingBatchStream;
use owhisper_interface::stream::StreamResponse;
use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef, SpawnErr};
use tracing::Instrument;

use super::accumulator::StreamBatchAccumulator;
use super::bootstrap::{notify_start_result, spawn_batch_task};
use super::{BatchParams, BatchRunOutput, format_user_friendly_error, session_span};
use crate::{BatchEvent, BatchRuntime};

const BATCH_STREAM_TIMEOUT_SECS: u64 = 30;

pub(super) async fn run_batch_streaming(
    runtime: Arc<dyn BatchRuntime>,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
) -> crate::Result<BatchRunOutput> {
    let span = session_span(&params.session_id);

    async {
        let (start_tx, start_rx) = tokio::sync::oneshot::channel::<crate::Result<()>>();
        let start_notifier = Arc::new(Mutex::new(Some(start_tx)));

        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<crate::Result<BatchRunOutput>>();
        let done_notifier = Arc::new(Mutex::new(Some(done_tx)));

        let args = BatchArgs {
            runtime: runtime.clone(),
            file_path: params.file_path,
            base_url: params.base_url,
            api_key: params.api_key,
            listen_params,
            start_notifier,
            done_notifier: done_notifier.clone(),
            session_id: params.session_id,
        };

        let batch_ref = match spawn_batch_actor(args).await {
            Ok(batch_ref) => {
                tracing::info!("batch actor spawned successfully");
                batch_ref
            }
            Err(err) => {
                let raw_error = format!("{err:?}");
                let message = format_user_friendly_error(&raw_error);
                tracing::error!(
                    error = %raw_error,
                    hyprnote.error.user_message = %message,
                    "batch supervisor spawn failed"
                );
                return Err(crate::BatchFailure::ActorSpawnFailed { message }.into());
            }
        };

        struct StopGuard(Option<ActorRef<BatchMsg>>);

        impl Drop for StopGuard {
            fn drop(&mut self) {
                if let Some(actor) = self.0.take() {
                    actor.stop(Some("listener2-core: run_batch dropped".to_string()));
                }
            }
        }

        let mut stop_guard = StopGuard(Some(batch_ref));

        match start_rx.await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                tracing::error!("batch actor reported start failure: {err}");
                return Err(err);
            }
            Err(_) => {
                tracing::error!("batch actor start notifier dropped before reporting result");
                return Err(crate::BatchFailure::StreamStartCancelled.into());
            }
        }

        match done_rx.await {
            Ok(Ok(output)) => {
                stop_guard.0 = None;
                Ok(output)
            }
            Ok(Err(err)) => Err(err),
            Err(_) => Err(crate::BatchFailure::StreamFinishedWithoutStatus.into()),
        }
    }
    .instrument(span)
    .await
}

fn is_completion_response(response: &StreamResponse) -> bool {
    matches!(
        response,
        StreamResponse::TranscriptResponse {
            from_finalize: true,
            ..
        } | StreamResponse::TerminalResponse { .. }
    )
}

fn provider_error_from_response(response: &StreamResponse) -> Option<(&str, &str, Option<i32>)> {
    let StreamResponse::ErrorResponse {
        provider,
        error_message,
        error_code,
    } = response
    else {
        return None;
    };

    Some((provider.as_str(), error_message.as_str(), *error_code))
}

#[allow(clippy::enum_variant_names)]
pub(super) enum BatchMsg {
    StreamResponse {
        response: Box<StreamResponse>,
        percentage: f64,
    },
    StreamError(crate::BatchFailure),
    StreamEnded,
    StreamStartFailed(crate::BatchFailure),
}

pub(super) type BatchStartNotifier =
    Arc<Mutex<Option<tokio::sync::oneshot::Sender<crate::Result<()>>>>>;
type BatchDoneNotifier =
    Arc<Mutex<Option<tokio::sync::oneshot::Sender<crate::Result<BatchRunOutput>>>>>;

#[derive(Clone)]
pub(super) struct BatchArgs {
    pub(super) runtime: Arc<dyn BatchRuntime>,
    pub(super) file_path: String,
    pub(super) base_url: String,
    pub(super) api_key: String,
    pub(super) listen_params: owhisper_interface::ListenParams,
    pub(super) start_notifier: BatchStartNotifier,
    pub(super) done_notifier: BatchDoneNotifier,
    pub(super) session_id: String,
}

struct BatchState {
    runtime: Arc<dyn BatchRuntime>,
    session_id: String,
    rx_task: tokio::task::JoinHandle<()>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    done_notifier: BatchDoneNotifier,
    final_result: Option<crate::Result<BatchRunOutput>>,
    accumulator: StreamBatchAccumulator,
}

impl BatchState {
    fn emit_streamed(&self, response: StreamResponse, percentage: f64) {
        self.runtime.emit(BatchEvent::BatchResponseStreamed {
            session_id: self.session_id.clone(),
            response,
            percentage,
        });
    }
}

struct BatchActor;

impl BatchActor {
    fn name() -> ActorName {
        "batch_actor".into()
    }
}

async fn spawn_batch_actor(args: BatchArgs) -> Result<ActorRef<BatchMsg>, SpawnErr> {
    let (batch_ref, _) = Actor::spawn(Some(BatchActor::name()), BatchActor, args).await?;
    Ok(batch_ref)
}

#[ractor::async_trait]
impl Actor for BatchActor {
    type Msg = BatchMsg;
    type State = BatchState;
    type Arguments = BatchArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let (rx_task, shutdown_tx) = spawn_batch_task(args.clone(), myself).await?;

        Ok(BatchState {
            runtime: args.runtime,
            session_id: args.session_id,
            rx_task,
            shutdown_tx: Some(shutdown_tx),
            done_notifier: args.done_notifier,
            final_result: None,
            accumulator: StreamBatchAccumulator::new(),
        })
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

        let final_result = state.final_result.take().unwrap_or_else(|| {
            Err(crate::BatchFailure::StreamStoppedWithoutCompletionSignal.into())
        });
        notify_done_result(&state.done_notifier, final_result);

        Ok(())
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            BatchMsg::StreamResponse {
                response,
                percentage,
            } => {
                tracing::info!("batch stream response received");
                state.accumulator.observe(&response);
                state.emit_streamed(*response, percentage);
            }
            BatchMsg::StreamStartFailed(error) => {
                tracing::error!("batch_stream_start_failed: {}", error);
                state.final_result = Some(Err(error.clone().into()));
                myself.stop(Some(format!("batch_stream_start_failed: {}", error)));
            }
            BatchMsg::StreamError(error) => {
                tracing::error!("batch_stream_error: {}", error);
                state.final_result = Some(Err(error.clone().into()));
                myself.stop(None);
            }
            BatchMsg::StreamEnded => {
                tracing::info!("batch_stream_ended");
                state.final_result = Some(Ok(
                    std::mem::take(&mut state.accumulator).finish(&state.session_id)
                ));
                myself.stop(None);
            }
        }

        Ok(())
    }
}

fn notify_done_result(notifier: &BatchDoneNotifier, result: crate::Result<BatchRunOutput>) {
    if let Ok(mut guard) = notifier.lock()
        && let Some(sender) = guard.take()
    {
        let _ = sender.send(result);
    }
}

pub(super) fn report_stream_start_failure(
    myself: &ActorRef<BatchMsg>,
    notifier: &BatchStartNotifier,
    error: &impl std::fmt::Debug,
    context: &str,
) {
    let raw_error = format!("{error:?}");
    let message = format_user_friendly_error(&raw_error);
    let failure = crate::BatchFailure::StreamStartFailed {
        message: message.clone(),
    };

    tracing::error!(
        error = %raw_error,
        hyprnote.error.user_message = %message,
        "{context}"
    );
    notify_start_result(notifier, Err(failure.clone().into()));
    let _ = myself.send_message(BatchMsg::StreamStartFailed(failure));
}

pub(super) async fn process_provider_stream(
    stream: StreamingBatchStream,
    myself: ActorRef<BatchMsg>,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    context: &str,
) {
    futures_util::pin_mut!(stream);
    process_stream_loop(&mut stream, myself, shutdown_rx, context, |event| {
        (event.response, event.percentage)
    })
    .await;
}

pub(super) async fn process_batch_stream<S, E>(
    mut listen_stream: std::pin::Pin<&mut S>,
    myself: ActorRef<BatchMsg>,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    audio_duration_secs: f64,
) where
    S: futures_util::Stream<Item = Result<StreamResponse, E>>,
    E: std::fmt::Debug,
{
    process_stream_loop(
        &mut listen_stream,
        myself,
        shutdown_rx,
        "batch stream",
        |response| {
            let percentage = compute_percentage(&response, audio_duration_secs);
            (response, percentage)
        },
    )
    .await;
}

async fn process_stream_loop<S, Item, E, F>(
    stream: &mut std::pin::Pin<&mut S>,
    myself: ActorRef<BatchMsg>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    context: &str,
    mut into_response: F,
) where
    S: futures_util::Stream<Item = Result<Item, E>>,
    E: std::fmt::Debug,
    F: FnMut(Item) -> (StreamResponse, f64),
{
    let mut response_count = 0;
    let response_timeout = Duration::from_secs(BATCH_STREAM_TIMEOUT_SECS);
    let mut completion_seen = false;

    loop {
        tracing::debug!(
            "{context}: waiting for next item (received {} so far)",
            response_count
        );

        tokio::select! {
            _ = &mut shutdown_rx => {
                tracing::info!("{context}: shutdown");
                return;
            }
            result = tokio::time::timeout(
                response_timeout,
                futures_util::StreamExt::next(stream),
            ) => {
                tracing::debug!("{context}: received result");
                match result {
                    Ok(Some(Ok(item))) => {
                        response_count += 1;
                        let (response, percentage) = into_response(item);

                        let is_from_finalize = matches!(
                            &response,
                            StreamResponse::TranscriptResponse { from_finalize, .. } if *from_finalize
                        );
                        let is_completion = is_completion_response(&response);

                        tracing::info!(
                            "{context}: response #{}{}",
                            response_count,
                            if is_from_finalize { " (from_finalize)" } else { "" }
                        );

                        if let Some((provider, error_message, error_code)) =
                            provider_error_from_response(&response)
                        {
                            tracing::error!(
                                hyprnote.stt.provider.name = %provider,
                                error.code = ?error_code,
                                error = %error_message,
                                hyprnote.response.count = response_count,
                                "{context} received provider error response"
                            );
                            let message = format_user_friendly_error(error_message);
                            send_actor_message(
                                &myself,
                                BatchMsg::StreamError(crate::BatchFailure::StreamError { message }),
                                context,
                                "stream error",
                            );
                            break;
                        }

                        send_actor_message(&myself, BatchMsg::StreamResponse {
                            response: Box::new(response),
                            percentage,
                        }, context, "stream response");

                        if is_completion {
                            completion_seen = true;
                            break;
                        }
                    }
                    Ok(Some(Err(err))) => {
                        let raw_error = format!("{err:?}");
                        let message = format_user_friendly_error(&raw_error);
                        tracing::error!(
                            error = %raw_error,
                            hyprnote.error.user_message = %message,
                            hyprnote.response.count = response_count,
                            "{context} stream error"
                        );
                        send_actor_message(
                            &myself,
                            BatchMsg::StreamError(crate::BatchFailure::StreamError { message }),
                            context,
                            "stream error",
                        );
                        break;
                    }
                    Ok(None) => {
                        if completion_seen {
                            tracing::info!(
                                hyprnote.response.count = response_count,
                                "{context} completed"
                            );
                            break;
                        }

                        tracing::error!(
                            hyprnote.response.count = response_count,
                            "{context} ended without completion signal"
                        );
                        send_actor_message(
                            &myself,
                            BatchMsg::StreamError(
                                crate::BatchFailure::StreamStoppedWithoutCompletionSignal,
                            ),
                            context,
                            "stream error",
                        );
                        break;
                    }
                    Err(elapsed) => {
                        tracing::warn!(
                            hyprnote.timeout.elapsed = ?elapsed,
                            hyprnote.response.count = response_count,
                            "{context} timeout"
                        );
                        send_actor_message(
                            &myself,
                            BatchMsg::StreamError(crate::BatchFailure::StreamTimeout),
                            context,
                            "timeout error",
                        );
                        break;
                    }
                }
            }
        }
    }

    if completion_seen {
        send_actor_message(&myself, BatchMsg::StreamEnded, context, "stream ended");
    }
    tracing::info!("{context}: processing loop exited");
}

fn send_actor_message(
    myself: &ActorRef<BatchMsg>,
    message: BatchMsg,
    context: &str,
    message_kind: &str,
) {
    if let Err(err) = myself.send_message(message) {
        tracing::error!(
            "{context}: failed to send {message_kind} message: {:?}",
            err
        );
    }
}

fn compute_percentage(response: &StreamResponse, audio_duration_secs: f64) -> f64 {
    match transcript_end_from_response(response) {
        Some(end) if audio_duration_secs > 0.0 => (end / audio_duration_secs).clamp(0.0, 1.0),
        _ => 0.0,
    }
}

fn transcript_end_from_response(response: &StreamResponse) -> Option<f64> {
    let StreamResponse::TranscriptResponse {
        start,
        duration,
        channel,
        ..
    } = response
    else {
        return None;
    };

    let mut end = (*start + *duration).max(0.0);

    for alternative in &channel.alternatives {
        for word in &alternative.words {
            if word.end.is_finite() {
                end = end.max(word.end);
            }
        }
    }

    if end.is_finite() { Some(end) } else { None }
}

#[cfg(test)]
mod test {
    use owhisper_interface::stream::{Alternatives, Channel, Metadata, ModelInfo};

    use super::*;

    #[test]
    fn completion_response_from_finalize() {
        let response = StreamResponse::TranscriptResponse {
            start: 0.0,
            duration: 0.1,
            is_final: true,
            speech_final: true,
            from_finalize: true,
            channel: Channel {
                alternatives: vec![Alternatives {
                    transcript: "hi".to_string(),
                    words: Vec::new(),
                    confidence: 1.0,
                    languages: Vec::new(),
                }],
            },
            metadata: Metadata {
                request_id: "r".to_string(),
                model_info: ModelInfo {
                    name: "".to_string(),
                    version: "".to_string(),
                    arch: "".to_string(),
                },
                model_uuid: "m".to_string(),
                extra: None,
            },
            channel_index: vec![0, 1],
        };

        assert!(is_completion_response(&response));
    }

    #[test]
    fn completion_response_terminal() {
        let response = StreamResponse::TerminalResponse {
            request_id: "r".to_string(),
            created: "now".to_string(),
            duration: 1.0,
            channels: 1,
        };

        assert!(is_completion_response(&response));
    }

    #[test]
    fn provider_error_extracts_fields() {
        let response = StreamResponse::ErrorResponse {
            error_code: Some(42),
            error_message: "nope".to_string(),
            provider: "x".to_string(),
        };

        let extracted = provider_error_from_response(&response);
        assert_eq!(extracted, Some(("x", "nope", Some(42))));
    }
}

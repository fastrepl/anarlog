use std::sync::{Arc, Mutex};
use std::time::Duration;

use owhisper_client::StreamingBatchStream;
use owhisper_interface::batch_stream::BatchStreamEvent;
use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef, SpawnErr};
use tracing::Instrument;

use super::super::accumulator::StreamBatchAccumulator;
use super::super::{BatchParams, BatchRunOutput, format_user_friendly_error, session_span};
use super::ProgressiveProvider;
use super::bootstrap::{notify_start_result, spawn_progressive_batch_task};
use crate::{BatchEvent, BatchRuntime};

const BATCH_STREAM_TIMEOUT_SECS: u64 = 30;

pub(super) async fn run_progressive_batch(
    runtime: Arc<dyn BatchRuntime>,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
    progressive_provider: ProgressiveProvider,
) -> crate::Result<BatchRunOutput> {
    let span = session_span(&params.session_id);
    let provider_label = progressive_provider.label().to_string();

    async {
        let (start_tx, start_rx) = tokio::sync::oneshot::channel::<crate::Result<()>>();
        let start_notifier = Arc::new(Mutex::new(Some(start_tx)));

        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<crate::Result<BatchRunOutput>>();
        let done_notifier = Arc::new(Mutex::new(Some(done_tx)));

        let args = BatchArgs {
            runtime: runtime.clone(),
            progressive_provider,
            provider_label: provider_label.clone(),
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
                return Err(crate::BatchFailure::ProgressiveActorSpawnFailed {
                    provider: provider_label.clone(),
                    message,
                }
                .into());
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
                return Err(crate::BatchFailure::ProgressiveStartCancelled.into());
            }
        }

        match done_rx.await {
            Ok(Ok(output)) => {
                stop_guard.0 = None;
                Ok(output)
            }
            Ok(Err(err)) => Err(err),
            Err(_) => Err(crate::BatchFailure::ProgressiveFinishedWithoutStatus.into()),
        }
    }
    .instrument(span)
    .await
}

fn is_completion_event(event: &BatchStreamEvent) -> bool {
    matches!(
        event,
        BatchStreamEvent::Result { .. } | BatchStreamEvent::Terminal { .. }
    )
}

fn provider_error_from_event(event: &BatchStreamEvent) -> Option<(&str, &str, Option<i32>)> {
    let BatchStreamEvent::Error {
        provider,
        error_message,
        error_code,
    } = event
    else {
        return None;
    };

    Some((provider.as_str(), error_message.as_str(), *error_code))
}

#[allow(clippy::enum_variant_names)]
pub(super) enum BatchMsg {
    StreamResponse { event: Box<BatchStreamEvent> },
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
    pub(super) progressive_provider: ProgressiveProvider,
    pub(super) provider_label: String,
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
    fn emit_streamed(&self, event: BatchStreamEvent) {
        self.runtime.emit(BatchEvent::BatchResponseStreamed {
            session_id: self.session_id.clone(),
            event,
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
        let (rx_task, shutdown_tx) = spawn_progressive_batch_task(args.clone(), myself).await?;

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
            Err(crate::BatchFailure::ProgressiveStoppedWithoutCompletionSignal.into())
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
            BatchMsg::StreamResponse { event } => {
                tracing::info!("batch stream response received");
                state.accumulator.observe(&event);
                state.emit_streamed(*event);
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
                let output = std::mem::take(&mut state.accumulator).finish(&state.session_id);
                state.final_result = Some(Ok(output));
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
    provider: &str,
    error: &impl std::fmt::Debug,
    context: &str,
) {
    let raw_error = format!("{error:?}");
    let message = format_user_friendly_error(&raw_error);
    let failure = crate::BatchFailure::ProgressiveStartFailed {
        provider: provider.to_string(),
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
    provider: &str,
    context: &str,
) {
    futures_util::pin_mut!(stream);
    process_stream_loop(
        &mut stream,
        myself,
        shutdown_rx,
        provider,
        context,
        1,
        std::convert::identity,
    )
    .await;
}

async fn process_stream_loop<S, Item, E, F>(
    stream: &mut std::pin::Pin<&mut S>,
    myself: ActorRef<BatchMsg>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    provider: &str,
    context: &str,
    expected_completions: usize,
    mut into_response: F,
) where
    S: futures_util::Stream<Item = Result<Item, E>>,
    E: std::fmt::Debug,
    F: FnMut(Item) -> BatchStreamEvent,
{
    let mut response_count = 0;
    let response_timeout = Duration::from_secs(BATCH_STREAM_TIMEOUT_SECS);
    let mut completions_seen: usize = 0;

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
                        let event = into_response(item);

                        let is_completion = is_completion_event(&event);

                        tracing::info!(
                            "{context}: response #{}{}",
                            response_count,
                            if matches!(&event, BatchStreamEvent::Result { .. }) {
                                " (result)"
                            } else {
                                ""
                            }
                        );

                        if let Some((provider, error_message, error_code)) =
                            provider_error_from_event(&event)
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
                                BatchMsg::StreamError(crate::BatchFailure::ProgressiveStreamError {
                                    provider: provider.to_string(),
                                    message,
                                }),
                                context,
                                "stream error",
                            );
                            break;
                        }

                        send_actor_message(
                            &myself,
                            BatchMsg::StreamResponse {
                                event: Box::new(event),
                            },
                            context,
                            "stream response",
                        );

                        if is_completion {
                            completions_seen += 1;
                            if completions_seen >= expected_completions {
                                break;
                            }
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
                            BatchMsg::StreamError(crate::BatchFailure::ProgressiveStreamError {
                                provider: provider.to_string(),
                                message,
                            }),
                            context,
                            "stream error",
                        );
                        break;
                    }
                    Ok(None) => {
                        if completions_seen >= expected_completions {
                            tracing::info!(
                                hyprnote.response.count = response_count,
                                "{context} completed"
                            );
                            break;
                        }

                        tracing::error!(
                            hyprnote.response.count = response_count,
                            hyprnote.completions.expected = expected_completions,
                            hyprnote.completions.seen = completions_seen,
                            "{context} ended without completion signal"
                        );
                        send_actor_message(
                            &myself,
                            BatchMsg::StreamError(
                                crate::BatchFailure::ProgressiveStoppedWithoutCompletionSignal,
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
                            BatchMsg::StreamError(crate::BatchFailure::ProgressiveStreamTimeout),
                            context,
                            "timeout error",
                        );
                        break;
                    }
                }
            }
        }
    }

    if completions_seen >= expected_completions {
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn completion_event_result() {
        let event = BatchStreamEvent::Result {
            response: owhisper_interface::batch::Response {
                metadata: serde_json::json!({}),
                results: owhisper_interface::batch::Results { channels: vec![] },
            },
        };
        assert!(is_completion_event(&event));
    }

    #[test]
    fn completion_event_terminal() {
        let event = BatchStreamEvent::Terminal {
            request_id: "req".into(),
            created: "now".into(),
            duration: 1.0,
            channels: 1,
        };
        assert!(is_completion_event(&event));
    }

    #[test]
    fn provider_error_extracts_fields() {
        let event = BatchStreamEvent::Error {
            provider: "deepgram".into(),
            error_message: "boom".into(),
            error_code: Some(429),
        };
        assert_eq!(
            provider_error_from_event(&event),
            Some(("deepgram", "boom", Some(429)))
        );
    }
}

use futures_util::StreamExt;
use owhisper_client::FinalizeHandle;
use owhisper_interface::stream::{Extra, StreamResponse};
use ractor::{ActorRef, rpc::CallResult};

use super::{FINALIZE_STREAM_TIMEOUT, LISTEN_STREAM_TIMEOUT, ListenerMsg};

const PROVIDER_RESPONSE_ACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);
const FINALIZE_HANDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);
const MAX_FINAL_RESPONSES: usize = 64;

pub(super) async fn process_stream<S, E, H>(
    mut listen_stream: std::pin::Pin<&mut S>,
    handle: H,
    myself: ActorRef<ListenerMsg>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    offset_secs: f64,
    extra: Extra,
) -> Vec<StreamResponse>
where
    S: futures_util::Stream<Item = Result<StreamResponse, E>>,
    E: std::fmt::Debug,
    H: FinalizeHandle,
{
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                return collect_final_responses(
                    listen_stream.as_mut(),
                    &handle,
                    offset_secs,
                    &extra,
                    None,
                ).await;
            }
            result = tokio::time::timeout(LISTEN_STREAM_TIMEOUT, listen_stream.next()) => {
                match result {
                    Ok(Some(Ok(mut response))) => {
                        response.apply_offset(offset_secs);
                        response.set_extra(&extra);

                        let response_for_shutdown = response.clone();
                        let delivery = myself.call(
                            move |reply| ListenerMsg::StreamResponse(response, reply),
                            Some(PROVIDER_RESPONSE_ACK_TIMEOUT),
                        );
                        tokio::pin!(delivery);

                        tokio::select! {
                            biased;
                            result = &mut delivery => {
                                match result {
                                    Ok(CallResult::Success(())) => {}
                                    Ok(CallResult::Timeout) => {
                                        tracing::warn!("provider_response_ack_timed_out");
                                        myself.kill();
                                        return Vec::new();
                                    }
                                    Ok(CallResult::SenderError) | Err(_) => {
                                        tracing::warn!("actor_gone_breaking_stream_loop");
                                        return Vec::new();
                                    }
                                }
                            }
                            _ = &mut shutdown_rx => {
                                return collect_final_responses(
                                    listen_stream.as_mut(),
                                    &handle,
                                    offset_secs,
                                    &extra,
                                    Some(response_for_shutdown),
                                ).await;
                            }
                        }
                    }
                    Ok(Some(Err(error))) => {
                        let _ = myself.send_message(ListenerMsg::StreamError(format!("{error:?}")));
                        return Vec::new();
                    }
                    Ok(None) => {
                        let _ = myself.send_message(ListenerMsg::StreamEnded);
                        return Vec::new();
                    }
                    Err(elapsed) => {
                        let _ = myself.send_message(ListenerMsg::StreamTimeout(elapsed));
                        return Vec::new();
                    }
                }
            }
        }
    }
}

async fn collect_final_responses<S, E, H>(
    mut listen_stream: std::pin::Pin<&mut S>,
    handle: &H,
    offset_secs: f64,
    extra: &Extra,
    initial_response: Option<StreamResponse>,
) -> Vec<StreamResponse>
where
    S: futures_util::Stream<Item = Result<StreamResponse, E>>,
    E: std::fmt::Debug,
    H: FinalizeHandle,
{
    if tokio::time::timeout(FINALIZE_HANDLE_TIMEOUT, handle.finalize())
        .await
        .is_err()
    {
        tracing::warn!("provider_finalize_timed_out");
    }

    let expected_count = handle.expected_finalize_count();
    let mut responses = initial_response.into_iter().collect::<Vec<_>>();
    let mut finalize_count = responses
        .iter()
        .filter(|response| is_from_finalize(response))
        .count();

    if expected_count > 0 && finalize_count >= expected_count {
        return responses;
    }

    let finalize_timeout = tokio::time::sleep(FINALIZE_STREAM_TIMEOUT);
    tokio::pin!(finalize_timeout);

    loop {
        tokio::select! {
            _ = &mut finalize_timeout => {
                tracing::warn!(anarlog.timeout.reached = true, "break_timeout");
                break;
            }
            result = listen_stream.next() => {
                match result {
                    Some(Ok(mut response)) => {
                        if is_from_finalize(&response) {
                            finalize_count += 1;
                        }
                        response.apply_offset(offset_secs);
                        response.set_extra(extra);
                        responses.push(response);

                        if expected_count > 0 && finalize_count >= expected_count {
                            tracing::info!(finalize_count, expected_count, "break_from_finalize");
                            break;
                        }
                        if responses.len() >= MAX_FINAL_RESPONSES {
                            tracing::warn!(
                                collected = responses.len(),
                                expected_count,
                                "final_response_limit_reached"
                            );
                            break;
                        }
                    }
                    Some(Err(error)) => {
                        tracing::warn!(error.message = ?error, "break_from_finalize");
                        break;
                    }
                    None => {
                        tracing::info!(anarlog.stream.ended = true, "break_from_finalize");
                        break;
                    }
                }
            }
        }
    }

    responses
}

fn is_from_finalize(response: &StreamResponse) -> bool {
    matches!(
        response,
        StreamResponse::TranscriptResponse {
            from_finalize: true,
            ..
        }
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use ractor::{Actor, ActorProcessingErr, ActorRef};

    use super::*;
    use crate::actors::{ListenerAudioResult, ListenerConfigUpdate};

    struct ResponseProbe {
        delay: Duration,
        response_tx: tokio::sync::mpsc::UnboundedSender<()>,
    }

    struct StuckResponseProbe;

    #[ractor::async_trait]
    impl Actor for ResponseProbe {
        type Msg = ListenerMsg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            _args: Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(())
        }

        async fn handle(
            &self,
            _myself: ActorRef<Self::Msg>,
            message: Self::Msg,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            match message {
                ListenerMsg::StreamResponse(_, reply) => {
                    tokio::time::sleep(self.delay).await;
                    let _ = self.response_tx.send(());
                    let _ = reply.send(());
                }
                ListenerMsg::AudioSingle(_, reply) | ListenerMsg::AudioDual(_, _, reply) => {
                    let _ = reply.send(ListenerAudioResult::Accepted);
                }
                ListenerMsg::UpdateConfig(ListenerConfigUpdate { .. })
                | ListenerMsg::StreamError(_)
                | ListenerMsg::StreamEnded
                | ListenerMsg::StreamTimeout(_) => {}
            }
            Ok(())
        }
    }

    #[ractor::async_trait]
    impl Actor for StuckResponseProbe {
        type Msg = ListenerMsg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            _args: Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(())
        }

        async fn handle(
            &self,
            _myself: ActorRef<Self::Msg>,
            message: Self::Msg,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            match message {
                ListenerMsg::StreamResponse(_, reply) => {
                    let reply = reply;
                    std::future::pending::<()>().await;
                    drop(reply);
                }
                ListenerMsg::AudioSingle(_, reply) | ListenerMsg::AudioDual(_, _, reply) => {
                    let _ = reply.send(ListenerAudioResult::Accepted);
                }
                ListenerMsg::UpdateConfig(ListenerConfigUpdate { .. })
                | ListenerMsg::StreamError(_)
                | ListenerMsg::StreamEnded
                | ListenerMsg::StreamTimeout(_) => {}
            }
            Ok(())
        }
    }

    struct NoopFinalizeHandle;

    impl FinalizeHandle for NoopFinalizeHandle {
        async fn finalize(&self) {}

        fn expected_finalize_count(&self) -> usize {
            0
        }
    }

    struct StuckFinalizeHandle;

    impl FinalizeHandle for StuckFinalizeHandle {
        async fn finalize(&self) {
            std::future::pending::<()>().await;
        }

        fn expected_finalize_count(&self) -> usize {
            0
        }
    }

    struct ResponseFinalizeHandle {
        response_tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<Result<StreamResponse, ()>>>>,
    }

    impl FinalizeHandle for ResponseFinalizeHandle {
        async fn finalize(&self) {
            if let Some(response_tx) = self.response_tx.lock().unwrap().take() {
                let _ = response_tx.send(Ok(terminal_response()));
            }
        }

        fn expected_finalize_count(&self) -> usize {
            0
        }
    }

    fn terminal_response() -> StreamResponse {
        StreamResponse::TerminalResponse {
            request_id: "request".to_string(),
            created: "now".to_string(),
            duration: 0.0,
            channels: 1,
        }
    }

    fn extra() -> Extra {
        Extra {
            started_unix_millis: 0,
        }
    }

    #[tokio::test]
    async fn provider_responses_wait_for_actor_acknowledgement() {
        let (response_tx, mut response_rx) = tokio::sync::mpsc::unbounded_channel();
        let (actor, actor_handle) = Actor::spawn(
            None,
            ResponseProbe {
                delay: Duration::from_millis(20),
                response_tx,
            },
            (),
        )
        .await
        .unwrap();
        let responses = futures_util::stream::iter(
            (0..5).map(|_| Ok::<StreamResponse, ()>(terminal_response())),
        );
        futures_util::pin_mut!(responses);
        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let started_at = Instant::now();

        let final_responses = process_stream(
            responses,
            NoopFinalizeHandle,
            actor.clone(),
            shutdown_rx,
            0.0,
            extra(),
        )
        .await;

        assert!(final_responses.is_empty());
        assert!(started_at.elapsed() >= Duration::from_millis(100));
        for _ in 0..5 {
            response_rx.recv().await.unwrap();
        }

        actor.stop(None);
        let _ = actor_handle.await;
    }

    #[tokio::test]
    async fn provider_response_ack_timeout_terminates_stuck_actor() {
        let (actor, actor_handle) = Actor::spawn(None, StuckResponseProbe, ()).await.unwrap();
        let responses = futures_util::stream::iter([Ok::<StreamResponse, ()>(terminal_response())]);
        futures_util::pin_mut!(responses);
        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let final_responses = tokio::time::timeout(
            PROVIDER_RESPONSE_ACK_TIMEOUT + Duration::from_secs(1),
            process_stream(
                responses,
                NoopFinalizeHandle,
                actor,
                shutdown_rx,
                0.0,
                extra(),
            ),
        )
        .await
        .expect("stuck provider response should reach its acknowledgement timeout");

        assert!(final_responses.is_empty());
        tokio::time::timeout(Duration::from_secs(1), actor_handle)
            .await
            .expect("stuck listener should be terminated after the response timeout")
            .unwrap();
    }

    #[tokio::test]
    async fn finalize_responses_are_returned_without_reentering_stopped_actor() {
        let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (actor, actor_handle) = Actor::spawn(
            None,
            ResponseProbe {
                delay: Duration::ZERO,
                response_tx: probe_tx,
            },
            (),
        )
        .await
        .unwrap();
        let (response_tx, response_rx) = tokio::sync::mpsc::unbounded_channel();
        let responses = tokio_stream::wrappers::UnboundedReceiverStream::new(response_rx);
        futures_util::pin_mut!(responses);
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        shutdown_tx.send(()).unwrap();

        let final_responses = process_stream(
            responses,
            ResponseFinalizeHandle {
                response_tx: Mutex::new(Some(response_tx)),
            },
            actor.clone(),
            shutdown_rx,
            0.0,
            extra(),
        )
        .await;

        assert_eq!(final_responses, vec![terminal_response()]);
        assert!(
            tokio::time::timeout(Duration::from_millis(50), probe_rx.recv())
                .await
                .is_err()
        );

        actor.stop(None);
        let _ = actor_handle.await;
    }

    #[tokio::test]
    async fn hung_provider_finalize_has_a_deadline() {
        let responses = futures_util::stream::empty::<Result<StreamResponse, ()>>();
        futures_util::pin_mut!(responses);
        let started_at = Instant::now();

        let final_responses = tokio::time::timeout(
            FINALIZE_HANDLE_TIMEOUT + Duration::from_secs(1),
            collect_final_responses(responses, &StuckFinalizeHandle, 0.0, &extra(), None),
        )
        .await
        .expect("provider finalize should be cancelled at its deadline");

        assert!(final_responses.is_empty());
        assert!(started_at.elapsed() >= FINALIZE_HANDLE_TIMEOUT);
    }

    #[tokio::test]
    async fn final_response_collection_is_bounded() {
        let responses = futures_util::stream::iter(
            (0..MAX_FINAL_RESPONSES + 10).map(|_| Ok::<StreamResponse, ()>(terminal_response())),
        );
        futures_util::pin_mut!(responses);

        let final_responses =
            collect_final_responses(responses, &NoopFinalizeHandle, 0.0, &extra(), None).await;

        assert_eq!(final_responses.len(), MAX_FINAL_RESPONSES);
    }
}

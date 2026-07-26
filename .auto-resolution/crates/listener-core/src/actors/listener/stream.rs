use futures_util::StreamExt;
use owhisper_client::FinalizeHandle;
use owhisper_interface::stream::{Extra, StreamResponse};
use ractor::ActorRef;

use super::{FINALIZE_STREAM_TIMEOUT, LISTEN_STREAM_TIMEOUT, ListenerMsg};

pub(super) async fn process_stream<S, E, H>(
    mut listen_stream: std::pin::Pin<&mut S>,
    handle: H,
    myself: ActorRef<ListenerMsg>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    offset_secs: f64,
    extra: Extra,
) where
    S: futures_util::Stream<Item = Result<StreamResponse, E>>,
    E: std::fmt::Debug,
    H: FinalizeHandle,
{
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                handle.finalize().await;

                let finalize_timeout = tokio::time::sleep(FINALIZE_STREAM_TIMEOUT);
                tokio::pin!(finalize_timeout);

                let expected_count = handle.expected_finalize_count();
                let mut finalize_count = 0usize;

                loop {
                    tokio::select! {
                        _ = &mut finalize_timeout => {
                            tracing::warn!(hyprnote.timeout.reached = true, "break_timeout");
                            break;
                        }
                        result = listen_stream.next() => {
                            match result {
                                Some(Ok(mut response)) => {
                                    let is_from_finalize = if let StreamResponse::TranscriptResponse { from_finalize, .. } = &response {
                                        *from_finalize
                                    } else {
                                        false
                                    };

                                    if is_from_finalize {
                                        finalize_count += 1;
                                    }

                                    response.apply_offset(offset_secs);
                                    response.set_extra(&extra);

                                    if myself.send_message(ListenerMsg::StreamResponse(response)).is_err() {
                                        tracing::debug!("actor_gone_during_finalize");
                                        break;
                                    }

                                    if finalize_count >= expected_count {
                                        tracing::info!(finalize_count, expected_count, "break_from_finalize");
                                        break;
                                    }
                                }
                                Some(Err(e)) => {
                                    tracing::warn!(error.message = ?e, "break_from_finalize");
                                    break;
                                }
                                None => {
                                    tracing::info!(hyprnote.stream.ended = true, "break_from_finalize");
                                    break;
                                }
                            }
                        }
                    }
                }
                break;
            }
            result = tokio::time::timeout(LISTEN_STREAM_TIMEOUT, listen_stream.next()) => {
                match result {
                    Ok(Some(Ok(mut response))) => {
                        response.apply_offset(offset_secs);
                        response.set_extra(&extra);

                        if myself.send_message(ListenerMsg::StreamResponse(response)).is_err() {
                            tracing::warn!("actor_gone_breaking_stream_loop");
                            break;
                        }
                    }
                    Ok(Some(Err(e))) => {
                        let _ = myself.send_message(ListenerMsg::StreamError(format!("{:?}", e)));
                        break;
                    }
                    Ok(None) => {
                        let _ = myself.send_message(ListenerMsg::StreamEnded);
                        break;
                    }
                    Err(elapsed) => {
                        let _ = myself.send_message(ListenerMsg::StreamTimeout(elapsed));
                        break;
                    }
                }
            }
        }
    }
}

use axum::extract::ws::Message;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

use super::super::handler::UPSTREAM_READY_TIMEOUT;
use super::super::pending::MAX_PENDING_QUEUE_BYTES;
use super::super::types::{
    ClientBinaryMessage, ClientBinaryMessageMapper, ClientMessageFilter, ClientReceiver,
    ClientSender, DEFAULT_CLOSE_CODE, ReadyNotifier, ReadyWaiter, ShutdownSignal, UpstreamEvent,
    UpstreamReceiver, UpstreamSender, convert, wait_until_ready,
};
use super::coordinator::SplitEvent;
use super::payload::RewrittenSplitResponse;

/// Upstream sink that buffers payloads until the upstream has acknowledged readiness.
pub(super) struct GatedUpstream {
    inner: UpstreamSender,
    ready: ReadyWaiter,
    queued: Vec<TungsteniteMessage>,
    queued_bytes: usize,
}

impl GatedUpstream {
    pub(super) fn new(inner: UpstreamSender, ready: ReadyWaiter) -> Self {
        Self {
            inner,
            ready,
            queued: Vec::new(),
            queued_bytes: 0,
        }
    }

    pub(super) fn is_ready(&self) -> bool {
        *self.ready.borrow()
    }

    pub(super) async fn wait_ready(&mut self) {
        wait_until_ready(&mut self.ready).await
    }

    pub(super) async fn send(&mut self, message: TungsteniteMessage) -> Result<(), ()> {
        if self.is_ready() {
            return self.flush(Some(message)).await;
        }
        let size = message.len();
        if self.queued_bytes + size > MAX_PENDING_QUEUE_BYTES {
            return Err(());
        }
        self.queued_bytes += size;
        self.queued.push(message);
        Ok(())
    }

    pub(super) async fn send_now(&mut self, message: TungsteniteMessage) -> Result<(), ()> {
        self.inner.send(message).await.map_err(|_| ())
    }

    pub(super) async fn flush(&mut self, tail: Option<TungsteniteMessage>) -> Result<(), ()> {
        let queued = std::mem::take(&mut self.queued);
        self.queued_bytes = 0;
        for message in queued.into_iter().chain(tail) {
            self.inner.send(message).await.map_err(|_| ())?;
        }
        Ok(())
    }
}

const SAMPLE_BYTES: usize = 2;
const FRAME_BYTES: usize = SAMPLE_BYTES * 2;
const UPSTREAM_DISCONNECTED_REASON: &str = "upstream_disconnected";
const UPSTREAM_SEND_FAILED_REASON: &str = "upstream_send_failed";

fn proxy_debug_enabled() -> bool {
    std::env::var("LISTENER_DEBUG")
        .map(|value| !value.is_empty() && value != "0" && value != "false")
        .unwrap_or(false)
}

pub(super) fn deinterleave(interleaved: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let num_frames = interleaved.len() / FRAME_BYTES;
    let mut ch0 = Vec::with_capacity(num_frames * SAMPLE_BYTES);
    let mut ch1 = Vec::with_capacity(num_frames * SAMPLE_BYTES);

    for frame in interleaved.chunks_exact(FRAME_BYTES) {
        ch0.extend_from_slice(&frame[..SAMPLE_BYTES]);
        ch1.extend_from_slice(&frame[SAMPLE_BYTES..]);
    }

    (ch0, ch1)
}

fn upstream_disconnected_signal() -> ShutdownSignal {
    ShutdownSignal::Close {
        code: DEFAULT_CLOSE_CODE,
        reason: UPSTREAM_DISCONNECTED_REASON.to_string(),
    }
}

fn upstream_send_failed_signal() -> ShutdownSignal {
    ShutdownSignal::Close {
        code: DEFAULT_CLOSE_CODE,
        reason: UPSTREAM_SEND_FAILED_REASON.to_string(),
    }
}

fn upstream_receive_error_signal(error: &impl std::fmt::Display) -> ShutdownSignal {
    ShutdownSignal::Close {
        code: DEFAULT_CLOSE_CODE,
        reason: format!("upstream_error: {error}"),
    }
}

pub(super) async fn send_text(client_tx: &mut ClientSender, text: String) -> bool {
    client_tx.send(Message::Text(text.into())).await.is_ok()
}

pub(super) async fn send_rewritten(
    client_tx: &mut ClientSender,
    response: RewrittenSplitResponse,
) -> bool {
    let Some(text) = response.into_text() else {
        return true;
    };

    send_text(client_tx, text).await
}

pub(super) async fn relay_client_to_upstreams(
    mut client_rx: ClientReceiver,
    mut mic_tx: GatedUpstream,
    mut spk_tx: GatedUpstream,
    client_message_filters: [Option<ClientMessageFilter>; 2],
    client_binary_message_mapper: Option<ClientBinaryMessageMapper>,
    shutdown_tx: tokio::sync::broadcast::Sender<ShutdownSignal>,
    event_tx: tokio::sync::mpsc::Sender<SplitEvent>,
) {
    let mut shutdown_rx = shutdown_tx.subscribe();
    let ready_deadline = tokio::time::sleep(UPSTREAM_READY_TIMEOUT);
    tokio::pin!(ready_deadline);

    loop {
        tokio::select! {
            biased;
            result = shutdown_rx.recv() => {
                if let Ok(signal) = result
                    && let ShutdownSignal::Close { code, reason } = signal {
                        let close = convert::to_tungstenite_close(code, reason);
                        let _ = mic_tx.send_now(close.clone()).await;
                        let _ = spk_tx.send_now(close).await;
                    }
                break;
            },
            _ = mic_tx.wait_ready(), if !mic_tx.is_ready() => {
                if mic_tx.flush(None).await.is_err() {
                    let _ = event_tx.send(SplitEvent::Fatal(upstream_send_failed_signal())).await;
                    break;
                }
            },
            _ = spk_tx.wait_ready(), if !spk_tx.is_ready() => {
                if spk_tx.flush(None).await.is_err() {
                    let _ = event_tx.send(SplitEvent::Fatal(upstream_send_failed_signal())).await;
                    break;
                }
            },
            _ = &mut ready_deadline, if !mic_tx.is_ready() || !spk_tx.is_ready() => {
                tracing::error!(
                    error.type = "upstream_ready_timeout",
                    anarlog.timeout_ms = UPSTREAM_READY_TIMEOUT.as_millis() as u64,
                    "upstream_ready_timeout"
                );
                let _ = event_tx
                    .send(SplitEvent::Fatal(ShutdownSignal::Close {
                        code: DEFAULT_CLOSE_CODE,
                        reason: "upstream_ready_timeout".to_string(),
                    }))
                    .await;
                break;
            },
            msg_opt = client_rx.next() => {
                let Some(msg_result) = msg_opt else {
                    let _ = event_tx.send(SplitEvent::ClientClosed).await;
                    break;
                };

                let msg = match msg_result {
                    Ok(msg) => msg,
                    Err(_) => {
                        let _ = event_tx.send(SplitEvent::ClientClosed).await;
                        break;
                    }
                };

                match msg {
                    Message::Binary(bytes) => {
                        if bytes.len() % FRAME_BYTES != 0 {
                            tracing::error!(
                                anarlog.payload.size_bytes = bytes.len(),
                                "invalid_stereo_frame_alignment"
                            );
                            let _ = event_tx
                                .send(SplitEvent::Fatal(ShutdownSignal::Close {
                                    code: DEFAULT_CLOSE_CODE,
                                    reason: "invalid_stereo_frame_alignment".to_string(),
                                }))
                                .await;
                            break;
                        }

                        let (mic, spk) = deinterleave(&bytes);
                        let to_upstream = |data: Vec<u8>| {
                            let mapped = match client_binary_message_mapper.as_ref() {
                                Some(mapper) => mapper(data)?,
                                None => ClientBinaryMessage::Binary(data),
                            };

                            Some(match mapped {
                                ClientBinaryMessage::Text(text) => TungsteniteMessage::Text(text.into()),
                                ClientBinaryMessage::Binary(data) => TungsteniteMessage::Binary(data.into()),
                            })
                        };

                        let Some(mic_message) = to_upstream(mic) else {
                            continue;
                        };
                        let Some(spk_message) = to_upstream(spk) else {
                            continue;
                        };

                        if mic_tx
                            .send(mic_message)
                            .await
                            .is_err()
                            || spk_tx.send(spk_message).await.is_err()
                        {
                            let _ = event_tx
                                .send(SplitEvent::Fatal(upstream_send_failed_signal()))
                                .await;
                            break;
                        }
                    }
                    Message::Text(text) => {
                        let text_str = text.to_string();
                        let is_finalize = matches!(
                            serde_json::from_str::<owhisper_interface::ControlMessage>(&text_str),
                            Ok(owhisper_interface::ControlMessage::Finalize)
                        );
                        let apply = |filter: &Option<ClientMessageFilter>| match filter {
                            Some(filter) => filter(text_str.clone()),
                            None => Some(text_str.clone()),
                        };
                        let mic_forwarded = apply(&client_message_filters[0]);
                        let spk_forwarded = apply(&client_message_filters[1]);
                        if mic_forwarded.is_none() && spk_forwarded.is_none() {
                            continue;
                        }

                        if is_finalize
                            && event_tx.send(SplitEvent::FinalizeRequested).await.is_err()
                        {
                            break;
                        }

                        let mut failed = false;
                        if let Some(text) = mic_forwarded {
                            failed |= mic_tx.send(TungsteniteMessage::Text(text.into())).await.is_err();
                        }
                        if let Some(text) = spk_forwarded {
                            failed |= spk_tx.send(TungsteniteMessage::Text(text.into())).await.is_err();
                        }
                        if failed {
                            let _ = event_tx
                                .send(SplitEvent::Fatal(upstream_send_failed_signal()))
                                .await;
                            break;
                        }
                    }
                    Message::Close(frame) => {
                        let (code, reason) = convert::extract_axum_close(frame, "client_closed");
                        let close = convert::to_tungstenite_close(code, reason);
                        let _ = mic_tx.send_now(close.clone()).await;
                        let _ = spk_tx.send_now(close).await;
                        let _ = event_tx.send(SplitEvent::ClientClosed).await;
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}

pub(super) async fn relay_upstream_to_events(
    upstream_rx: &mut UpstreamReceiver,
    channel: usize,
    event_tx: tokio::sync::mpsc::Sender<SplitEvent>,
    shutdown_tx: tokio::sync::broadcast::Sender<ShutdownSignal>,
    mut readiness: Option<(UpstreamEvent, ReadyNotifier)>,
    completion: Option<&UpstreamEvent>,
) {
    let mut shutdown_rx = shutdown_tx.subscribe();

    loop {
        tokio::select! {
            biased;
            _ = shutdown_rx.recv() => break,
            msg_opt = upstream_rx.next() => {
                let Some(msg_result) = msg_opt else {
                    let _ = event_tx
                        .send(SplitEvent::Fatal(upstream_disconnected_signal()))
                        .await;
                    break;
                };

                let msg = match msg_result {
                    Ok(msg) => msg,
                    Err(error) => {
                        let _ = event_tx
                            .send(SplitEvent::Fatal(upstream_receive_error_signal(&error)))
                            .await;
                        break;
                    }
                };

                match msg {
                    TungsteniteMessage::Text(text) => {
                        if readiness.as_ref().is_some_and(|(r, _)| r.matches(text.as_str()))
                            && let Some((_, notifier)) = readiness.take()
                        {
                            let _ = notifier.send(true);
                        }
                        if proxy_debug_enabled() {
                            tracing::info!(
                                anarlog.stream.channel = channel,
                                anarlog.payload.size_bytes = text.len(),
                                raw = %text,
                                "channel_split_upstream_text"
                            );
                        }
                        let completed = completion.is_some_and(|c| c.matches(text.as_str()));
                        if event_tx
                            .send(SplitEvent::Text {
                                channel,
                                raw: text.to_string(),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                        if completed {
                            let _ = event_tx
                                .send(SplitEvent::UpstreamClosed {
                                    channel,
                                    code: 1000,
                                    reason: "upstream_task_finished".to_string(),
                                })
                                .await;
                            break;
                        }
                    }
                    TungsteniteMessage::Close(frame) => {
                        let (code, reason) =
                            convert::extract_tungstenite_close(frame, "upstream_closed");
                        let _ = event_tx
                            .send(SplitEvent::UpstreamClosed {
                                channel,
                                code,
                                reason,
                            })
                            .await;
                        break;
                    }
                    TungsteniteMessage::Ping(_)
                    | TungsteniteMessage::Pong(_)
                    | TungsteniteMessage::Binary(_)
                    | TungsteniteMessage::Frame(_) => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::types::DEFAULT_CLOSE_CODE;

    #[test]
    fn deinterleave_basic() {
        let mic: [u8; 2] = [0x01, 0x00];
        let spk: [u8; 2] = [0x02, 0x00];
        let interleaved = [mic[0], mic[1], spk[0], spk[1]];

        let (ch0, ch1) = deinterleave(&interleaved);
        assert_eq!(ch0, mic);
        assert_eq!(ch1, spk);
    }

    #[test]
    fn deinterleave_multiple_frames() {
        let interleaved = [0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04, 0x00];

        let (ch0, ch1) = deinterleave(&interleaved);
        assert_eq!(ch0, [0x01, 0x00, 0x03, 0x00]);
        assert_eq!(ch1, [0x02, 0x00, 0x04, 0x00]);
    }

    #[test]
    fn deinterleave_empty() {
        let (ch0, ch1) = deinterleave(&[]);
        assert!(ch0.is_empty());
        assert!(ch1.is_empty());
    }

    #[test]
    fn upstream_disconnects_map_to_structured_close_signal() {
        assert!(matches!(
            upstream_disconnected_signal(),
            ShutdownSignal::Close { code, reason }
                if code == DEFAULT_CLOSE_CODE && reason == "upstream_disconnected"
        ));
    }

    #[test]
    fn upstream_send_failures_map_to_structured_close_signal() {
        assert!(matches!(
            upstream_send_failed_signal(),
            ShutdownSignal::Close { code, reason }
                if code == DEFAULT_CLOSE_CODE && reason == "upstream_send_failed"
        ));
    }

    #[test]
    fn upstream_receive_errors_preserve_the_transport_error_string() {
        let error = tokio_tungstenite::tungstenite::Error::ConnectionClosed;
        let ShutdownSignal::Close { code, reason } = upstream_receive_error_signal(&error) else {
            panic!("expected structured close signal");
        };

        assert_eq!(code, DEFAULT_CLOSE_CODE);
        assert_eq!(reason, format!("upstream_error: {error}"));
    }
}

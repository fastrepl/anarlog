use axum::extract::ws::Message;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

use super::super::types::{
    ClientMessageFilter, ClientReceiver, ClientSender, DEFAULT_CLOSE_CODE, ShutdownSignal,
    UpstreamReceiver, UpstreamSender, convert,
};
use super::coordinator::SplitEvent;
use super::payload::RewrittenSplitResponse;

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
    mut mic_tx: UpstreamSender,
    mut spk_tx: UpstreamSender,
    client_message_filter: Option<ClientMessageFilter>,
    shutdown_tx: tokio::sync::broadcast::Sender<ShutdownSignal>,
    event_tx: tokio::sync::mpsc::Sender<SplitEvent>,
) {
    let mut shutdown_rx = shutdown_tx.subscribe();

    loop {
        tokio::select! {
            biased;
            result = shutdown_rx.recv() => {
                if let Ok(signal) = result {
                    if let ShutdownSignal::Close { code, reason } = signal {
                        let close = convert::to_tungstenite_close(code, reason);
                        let _ = mic_tx.send(close.clone()).await;
                        let _ = spk_tx.send(close).await;
                    }
                }
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
                                hyprnote.payload.size_bytes = bytes.len(),
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
                        if mic_tx
                            .send(TungsteniteMessage::Binary(mic.into()))
                            .await
                            .is_err()
                            || spk_tx
                                .send(TungsteniteMessage::Binary(spk.into()))
                                .await
                                .is_err()
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
                        let forwarded = match client_message_filter.as_ref() {
                            Some(filter) => match filter(text_str) {
                                Some(text) => text,
                                None => continue,
                            },
                            None => text_str,
                        };

                        if is_finalize
                            && event_tx.send(SplitEvent::FinalizeRequested).await.is_err()
                        {
                            break;
                        }

                        let tung = TungsteniteMessage::Text(forwarded.into());
                        if mic_tx.send(tung.clone()).await.is_err() || spk_tx.send(tung).await.is_err()
                        {
                            let _ = event_tx
                                .send(SplitEvent::Fatal(upstream_send_failed_signal()))
                                .await;
                            break;
                        }
                    }
                    Message::Close(frame) => {
                        let (code, reason) = convert::extract_axum_close(frame, "client_closed");
                        let close = convert::to_tungstenite_close(code, reason);
                        let _ = mic_tx.send(close.clone()).await;
                        let _ = spk_tx.send(close).await;
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
                        if proxy_debug_enabled() {
                            tracing::info!(
                                hyprnote.stream.channel = channel,
                                hyprnote.payload.size_bytes = text.len(),
                                raw = %text,
                                "channel_split_upstream_text"
                            );
                        }
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

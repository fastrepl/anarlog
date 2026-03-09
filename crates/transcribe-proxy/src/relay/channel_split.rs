use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::Response;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use sentry::SentryFutureExt;
use tokio_tungstenite::tungstenite::ClientRequestBuilder;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::client::IntoClientRequest,
};

use owhisper_client::Provider;

use super::types::{
    ClientMessageFilter, InitialMessage, OnCloseCallback, ResponseTransformer, convert,
};

const SAMPLE_BYTES: usize = 2;
const FRAME_BYTES: usize = SAMPLE_BYTES * 2;

fn deinterleave(interleaved: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let num_frames = interleaved.len() / FRAME_BYTES;
    let mut ch0 = Vec::with_capacity(num_frames * SAMPLE_BYTES);
    let mut ch1 = Vec::with_capacity(num_frames * SAMPLE_BYTES);

    for frame in interleaved.chunks_exact(FRAME_BYTES) {
        ch0.extend_from_slice(&frame[..SAMPLE_BYTES]);
        ch1.extend_from_slice(&frame[SAMPLE_BYTES..]);
    }

    (ch0, ch1)
}

fn stamp_results_object(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    channel: i32,
    total: i32,
) {
    if obj.get("type").and_then(|t| t.as_str()) == Some("Results") {
        obj.insert(
            "channel_index".to_string(),
            serde_json::json!([channel, total]),
        );
    }
}

fn stamp_channel_index(text: &str, channel: i32, total: i32) -> Option<String> {
    let mut value: serde_json::Value = serde_json::from_str(text).ok()?;

    match &mut value {
        serde_json::Value::Object(obj) => stamp_results_object(obj, channel, total),
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let Some(obj) = item.as_object_mut() {
                    stamp_results_object(obj, channel, total);
                }
            }
        }
        _ => {}
    }

    serde_json::to_string(&value).ok()
}

#[derive(Clone)]
pub struct ChannelSplitProxy {
    mic_request: ClientRequestBuilder,
    spk_request: ClientRequestBuilder,
    initial_message: Option<InitialMessage>,
    response_transformer: Option<ResponseTransformer>,
    connect_timeout: Duration,
    on_close: Option<OnCloseCallback>,
    client_message_filter: Option<ClientMessageFilter>,
}

impl ChannelSplitProxy {
    pub fn new(
        upstream_request: ClientRequestBuilder,
        initial_message: Option<InitialMessage>,
        response_transformer: Option<ResponseTransformer>,
        connect_timeout: Duration,
        on_close: Option<OnCloseCallback>,
    ) -> Self {
        Self::with_split_requests(
            upstream_request.clone(),
            upstream_request,
            initial_message,
            response_transformer,
            connect_timeout,
            on_close,
        )
    }

    pub fn with_split_requests(
        mic_request: ClientRequestBuilder,
        spk_request: ClientRequestBuilder,
        initial_message: Option<InitialMessage>,
        response_transformer: Option<ResponseTransformer>,
        connect_timeout: Duration,
        on_close: Option<OnCloseCallback>,
    ) -> Self {
        Self {
            mic_request,
            spk_request,
            initial_message,
            response_transformer,
            connect_timeout,
            on_close,
            client_message_filter: None,
        }
    }

    pub fn with_client_message_filter(mut self, filter: ClientMessageFilter) -> Self {
        self.client_message_filter = Some(filter);
        self
    }

    async fn connect_upstream(
        request: &ClientRequestBuilder,
        timeout: Duration,
    ) -> Result<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, crate::ProxyError> {
        let mut req = request
            .clone()
            .into_client_request()
            .map_err(|e| crate::ProxyError::InvalidRequest(e.to_string()))?;
        hypr_observability::inject_current_trace_context(req.headers_mut());

        let result = tokio::time::timeout(timeout, connect_async(req)).await;
        match result {
            Ok(Ok((stream, _))) => Ok(stream),
            Ok(Err(e)) => Err(crate::ProxyError::ConnectionFailed(e.to_string())),
            Err(_) => Err(crate::ProxyError::ConnectionTimeout),
        }
    }

    pub async fn handle_upgrade(&self, ws: WebSocketUpgrade) -> Response<Body> {
        let proxy = self.clone();
        let hub = sentry::Hub::current();
        ws.on_upgrade(move |socket| {
            async move {
                if let Err(e) = proxy.handle(socket).await {
                    tracing::error!(error.message = %e, "channel_split_proxy_error");
                }
            }
            .bind_hub(sentry::Hub::new_from_top(hub))
        })
        .into_response()
    }

    async fn handle(&self, client_socket: WebSocket) -> Result<(), crate::ProxyError> {
        tracing::info!("connecting_to_upstream(channel_split)");
        let (mic_upstream, spk_upstream) = tokio::try_join!(
            Self::connect_upstream(&self.mic_request, self.connect_timeout),
            Self::connect_upstream(&self.spk_request, self.connect_timeout),
        )?;

        let start_time = Instant::now();

        Self::run_relay(
            client_socket,
            mic_upstream,
            spk_upstream,
            self.initial_message.clone(),
            self.response_transformer.clone(),
            self.client_message_filter.clone(),
        )
        .await;

        let duration = start_time.elapsed();
        if let Some(on_close) = &self.on_close {
            on_close(duration).await;
        }

        tracing::info!(
            hyprnote.duration_ms = %(duration.as_millis() as u64),
            "channel_split_proxy_closed"
        );

        Ok(())
    }

    async fn run_relay(
        client_socket: WebSocket,
        mic_upstream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
        spk_upstream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
        initial_message: Option<InitialMessage>,
        response_transformer: Option<ResponseTransformer>,
        client_message_filter: Option<ClientMessageFilter>,
    ) {
        let (mut mic_tx, mut mic_rx) = mic_upstream.split();
        let (mut spk_tx, mut spk_rx) = spk_upstream.split();
        let (mut client_tx, mut client_rx) = client_socket.split();

        if let Some(msg) = &initial_message {
            let tung_msg = TungsteniteMessage::Text(msg.as_str().into());
            if mic_tx.send(tung_msg.clone()).await.is_err() || spk_tx.send(tung_msg).await.is_err()
            {
                tracing::error!("channel_split_initial_message_send_failed");
                return;
            }
        }

        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let (merged_tx, mut merged_rx) = tokio::sync::mpsc::channel::<Message>(64);

        let client_to_upstreams = {
            let shutdown_tx = shutdown_tx.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            async move {
                loop {
                    tokio::select! {
                        biased;
                        _ = shutdown_rx.recv() => break,
                        msg_opt = client_rx.next() => {
                            let Some(Ok(msg)) = msg_opt else {
                                let _ = shutdown_tx.send(());
                                break;
                            };
                            match msg {
                                Message::Binary(bytes) => {
                                    let (mic, spk) = deinterleave(&bytes);
                                    if mic_tx.send(TungsteniteMessage::Binary(mic.into())).await.is_err()
                                        || spk_tx.send(TungsteniteMessage::Binary(spk.into())).await.is_err()
                                    {
                                        let _ = shutdown_tx.send(());
                                        break;
                                    }
                                }
                                Message::Text(text) => {
                                    let text_str = text.to_string();
                                    let forwarded = match client_message_filter.as_ref() {
                                        Some(filter) => match filter(text_str) {
                                            Some(s) => s,
                                            None => continue,
                                        },
                                        None => text_str,
                                    };
                                    let tung = TungsteniteMessage::Text(forwarded.into());
                                    if mic_tx.send(tung.clone()).await.is_err()
                                        || spk_tx.send(tung).await.is_err()
                                    {
                                        let _ = shutdown_tx.send(());
                                        break;
                                    }
                                }
                                Message::Close(frame) => {
                                    let (code, reason) = convert::extract_axum_close(frame, "client_closed");
                                    let close = convert::to_tungstenite_close(code, reason);
                                    let _ = mic_tx.send(close.clone()).await;
                                    let _ = spk_tx.send(close).await;
                                    let _ = shutdown_tx.send(());
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        };

        let mic_to_merged = Self::upstream_to_merged(
            &mut mic_rx,
            merged_tx.clone(),
            response_transformer.clone(),
            0,
            shutdown_tx.clone(),
        );

        let spk_to_merged = Self::upstream_to_merged(
            &mut spk_rx,
            merged_tx,
            response_transformer,
            1,
            shutdown_tx.clone(),
        );

        let merged_to_client = {
            let shutdown_tx = shutdown_tx.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            async move {
                loop {
                    tokio::select! {
                        biased;
                        _ = shutdown_rx.recv() => break,
                        msg_opt = merged_rx.recv() => {
                            match msg_opt {
                                Some(msg) => {
                                    if client_tx.send(msg).await.is_err() {
                                        let _ = shutdown_tx.send(());
                                        break;
                                    }
                                }
                                None => break,
                            }
                        }
                    }
                }
            }
        };

        tokio::join!(
            client_to_upstreams,
            mic_to_merged,
            spk_to_merged,
            merged_to_client,
        );
    }

    async fn upstream_to_merged(
        upstream_rx: &mut futures_util::stream::SplitStream<
            WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
        >,
        merged_tx: tokio::sync::mpsc::Sender<Message>,
        response_transformer: Option<ResponseTransformer>,
        channel: i32,
        shutdown_tx: tokio::sync::broadcast::Sender<()>,
    ) {
        let mut shutdown_rx = shutdown_tx.subscribe();
        let mut pending_error: Option<(u16, String)> = None;
        loop {
            tokio::select! {
                biased;
                _ = shutdown_rx.recv() => break,
                msg_opt = upstream_rx.next() => {
                    let Some(Ok(msg)) = msg_opt else {
                        if let Some((code, reason)) = pending_error {
                            let _ = merged_tx.send(convert::to_axum_close(code, reason)).await;
                        }
                        let _ = shutdown_tx.send(());
                        break;
                    };
                    if let TungsteniteMessage::Text(text) = msg {
                        if let Some(upstream_err) = Provider::detect_any_error(text.as_bytes()) {
                            tracing::warn!(
                                channel = channel,
                                error_code = upstream_err.http_code,
                                provider_code = ?upstream_err.provider_code,
                                error_message = %upstream_err.message,
                                "upstream_error_detected"
                            );
                            pending_error = Some((
                                upstream_err.to_ws_close_code(),
                                upstream_err.message.clone(),
                            ));
                        }

                        let transformed = match &response_transformer {
                            Some(t) => match t(text.as_str()) {
                                Some(s) => s,
                                None => continue,
                            },
                            None => text.to_string(),
                        };
                        if let Some(stamped) = stamp_channel_index(&transformed, channel, 2) {
                            let _ = merged_tx.send(Message::Text(stamped.into())).await;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deinterleave_basic() {
        let mic: [u8; 2] = [0x01, 0x00];
        let spk: [u8; 2] = [0x02, 0x00];
        let interleaved = [mic[0], mic[1], spk[0], spk[1]];

        let (ch0, ch1) = deinterleave(&interleaved);
        assert_eq!(ch0, mic);
        assert_eq!(ch1, spk);
    }

    #[test]
    fn test_deinterleave_multiple_frames() {
        let interleaved = [0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04, 0x00];

        let (ch0, ch1) = deinterleave(&interleaved);
        assert_eq!(ch0, [0x01, 0x00, 0x03, 0x00]);
        assert_eq!(ch1, [0x02, 0x00, 0x04, 0x00]);
    }

    #[test]
    fn test_deinterleave_empty() {
        let (ch0, ch1) = deinterleave(&[]);
        assert!(ch0.is_empty());
        assert!(ch1.is_empty());
    }

    #[test]
    fn test_stamp_channel_index_results() {
        let input = r#"{"type":"Results","channel_index":[0,1],"start":0.0}"#;
        let result = stamp_channel_index(input, 1, 2).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["channel_index"], serde_json::json!([1, 2]));
    }

    #[test]
    fn test_stamp_channel_index_non_results() {
        let input = r#"{"type":"Metadata","request_id":"abc"}"#;
        let result = stamp_channel_index(input, 1, 2).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("channel_index").is_none());
    }

    #[test]
    fn test_stamp_channel_index_array() {
        let input = r#"[{"type":"Results","start":0.0},{"type":"Results","start":1.0},{"type":"Metadata"}]"#;
        let result = stamp_channel_index(input, 0, 2).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed[0]["channel_index"], serde_json::json!([0, 2]));
        assert_eq!(parsed[1]["channel_index"], serde_json::json!([0, 2]));
        assert!(parsed[2].get("channel_index").is_none());
    }

    #[test]
    fn test_stamp_channel_index_invalid_json() {
        assert!(stamp_channel_index("not json", 0, 2).is_none());
    }
}

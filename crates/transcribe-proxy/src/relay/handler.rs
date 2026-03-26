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

use super::builder::WebSocketProxyBuilder;
use super::pending::{FlushError, PendingState, QueuedPayload};
use super::types::{
    ClientMessageFilter, ClientReceiver, ClientSender, ControlMessageTypes, DEFAULT_CLOSE_CODE,
    FirstMessageTransformer, InitialMessage, OnCloseCallback, ResponseTransformer, ShutdownSignal,
    UpstreamReceiver, UpstreamSender, convert, is_control_message,
};

#[derive(Clone)]
pub struct WebSocketProxy {
    upstream_request: ClientRequestBuilder,
    control_message_types: Option<ControlMessageTypes>,
    transform_first_message: Option<FirstMessageTransformer>,
    initial_message: Option<InitialMessage>,
    response_transformer: Option<ResponseTransformer>,
    connect_timeout: Duration,
    on_close: Option<OnCloseCallback>,
    client_message_filter: Option<ClientMessageFilter>,
}

impl WebSocketProxy {
    pub(crate) fn new(
        upstream_request: ClientRequestBuilder,
        control_message_types: Option<ControlMessageTypes>,
        transform_first_message: Option<FirstMessageTransformer>,
        initial_message: Option<InitialMessage>,
        response_transformer: Option<ResponseTransformer>,
        connect_timeout: Duration,
        on_close: Option<OnCloseCallback>,
        client_message_filter: Option<ClientMessageFilter>,
    ) -> Self {
        Self {
            upstream_request,
            control_message_types,
            transform_first_message,
            initial_message,
            response_transformer,
            connect_timeout,
            on_close,
            client_message_filter,
        }
    }

    pub fn builder() -> WebSocketProxyBuilder {
        WebSocketProxyBuilder::default()
    }

    async fn connect_upstream(
        &self,
    ) -> Result<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, crate::ProxyError> {
        let mut req = self
            .upstream_request
            .clone()
            .into_client_request()
            .map_err(|e| crate::ProxyError::InvalidRequest(e.to_string()))?;
        hypr_observability::inject_current_trace_context(req.headers_mut());

        let connect_start = Instant::now();
        tracing::info!("upstream_connect_started");

        let upstream_result = tokio::time::timeout(self.connect_timeout, connect_async(req)).await;

        match upstream_result {
            Ok(Ok((stream, _))) => {
                tracing::info!(
                    hyprnote.duration_ms = connect_start.elapsed().as_millis() as u64,
                    "upstream_connect_succeeded"
                );
                Ok(stream)
            }
            Ok(Err(e)) => {
                tracing::error!(
                    error.type = "upstream_connect_failed",
                    error = %e,
                    hyprnote.duration_ms = connect_start.elapsed().as_millis() as u64,
                    "upstream_connect_failed"
                );
                Err(crate::ProxyError::ConnectionFailed(e.to_string()))
            }
            Err(_) => {
                tracing::error!(
                    error.type = "upstream_connect_timeout",
                    hyprnote.timeout_ms = self.connect_timeout.as_millis() as u64,
                    "upstream_connect_timeout"
                );
                Err(crate::ProxyError::ConnectionTimeout)
            }
        }
    }

    pub async fn handle(&self, client_socket: WebSocket) -> Result<(), crate::ProxyError> {
        let upstream_stream = self.connect_upstream().await?;

        Self::run_proxy_loop(
            client_socket,
            upstream_stream,
            self.control_message_types.clone(),
            self.transform_first_message.clone(),
            self.initial_message.clone(),
            self.response_transformer.clone(),
            self.on_close.clone(),
            self.client_message_filter.clone(),
        )
        .await;

        Ok(())
    }

    pub async fn handle_upgrade(&self, ws: WebSocketUpgrade) -> Response<Body> {
        let proxy = self.clone();
        let hub = sentry::Hub::current();
        ws.on_upgrade(move |socket| {
            async move {
                if let Err(e) = proxy.handle(socket).await {
                    tracing::error!(
                        error = %e,
                        "websocket_proxy_error: {}",
                        e
                    );
                }
            }
            .bind_hub(sentry::Hub::new_from_top(hub))
        })
        .into_response()
    }

    async fn run_proxy_loop(
        client_socket: WebSocket,
        upstream_stream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
        control_message_types: Option<ControlMessageTypes>,
        transform_first_message: Option<FirstMessageTransformer>,
        initial_message: Option<InitialMessage>,
        response_transformer: Option<ResponseTransformer>,
        on_close: Option<OnCloseCallback>,
        client_message_filter: Option<ClientMessageFilter>,
    ) {
        let start_time = Instant::now();

        let (upstream_sender, upstream_receiver) = upstream_stream.split();
        let (client_sender, client_receiver) = client_socket.split();

        let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel::<ShutdownSignal>(1);
        let shutdown_rx2 = shutdown_tx.subscribe();

        let client_to_upstream = Self::run_client_to_upstream(
            client_receiver,
            upstream_sender,
            shutdown_tx.clone(),
            shutdown_rx,
            control_message_types,
            transform_first_message,
            initial_message,
            client_message_filter,
        );

        let upstream_to_client = Self::run_upstream_to_client(
            upstream_receiver,
            client_sender,
            shutdown_tx.clone(),
            shutdown_rx2,
            response_transformer,
        );

        let _ = tokio::join!(client_to_upstream, upstream_to_client);

        let duration = start_time.elapsed();
        if let Some(on_close) = on_close {
            on_close(duration).await;
        }

        tracing::info!(
            hyprnote.duration_ms = duration.as_millis() as u64,
            "websocket_proxy_connection_closed"
        );
    }

    async fn process_data_message(
        pending: &mut PendingState,
        data: Vec<u8>,
        is_text: bool,
        control_types: &Option<ControlMessageTypes>,
        shutdown_tx: &tokio::sync::broadcast::Sender<ShutdownSignal>,
        upstream_sender: &mut UpstreamSender,
    ) -> bool {
        let is_control = control_types
            .as_ref()
            .is_some_and(|types| is_control_message(&data, types));
        let size = data.len();
        let queued = QueuedPayload { data, is_text };

        if let Err(reason) = pending.enqueue(queued, is_control) {
            tracing::warn!(
                error = %reason,
                hyprnote.payload.size_bytes = %size,
                hyprnote.ws.is_control_message = %is_control,
                "pending_queue_enqueue_failed"
            );
            let _ = shutdown_tx.send(ShutdownSignal::Close {
                code: DEFAULT_CLOSE_CODE,
                reason: reason.to_string(),
            });
            return true;
        }

        if let Err(e) = pending.flush_to(upstream_sender).await {
            match e {
                FlushError::SendFailed => {
                    tracing::error!(
                        error.type = "upstream_send_failed",
                        error = ?e,
                        "pending_flush_failed"
                    );
                    let _ = shutdown_tx.send(ShutdownSignal::Abort);
                }
                FlushError::InvalidUtf8 => {
                    tracing::error!(
                        error.type = "invalid_utf8_in_message",
                        error = ?e,
                        "pending_flush_failed"
                    );
                    let _ = shutdown_tx.send(ShutdownSignal::Close {
                        code: DEFAULT_CLOSE_CODE,
                        reason: "invalid_utf8_in_message".to_string(),
                    });
                }
            }
            return true;
        }

        false
    }

    async fn run_client_to_upstream(
        mut client_receiver: ClientReceiver,
        mut upstream_sender: UpstreamSender,
        shutdown_tx: tokio::sync::broadcast::Sender<ShutdownSignal>,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<ShutdownSignal>,
        control_types: Option<ControlMessageTypes>,
        mut first_msg_transformer: Option<FirstMessageTransformer>,
        initial_message: Option<InitialMessage>,
        client_message_filter: Option<ClientMessageFilter>,
    ) {
        let mut pending = PendingState::default();

        if let Some(msg) = initial_message {
            if let Err(e) = upstream_sender
                .send(TungsteniteMessage::Text(msg.as_str().into()))
                .await
            {
                tracing::error!(error = ?e, "initial_message_send_failed");
                let _ = shutdown_tx.send(ShutdownSignal::Abort);
                return;
            }
            tracing::debug!("initial_message_sent");
        }

        loop {
            tokio::select! {
                biased;

                result = shutdown_rx.recv() => {
                    if let Ok(signal) = result {
                        if let ShutdownSignal::Close { code, reason } = signal {
                            let _ = upstream_sender.send(convert::to_tungstenite_close(code, reason)).await;
                        }
                    }
                    break;
                }

                msg_opt = client_receiver.next() => {
                    let Some(msg_result) = msg_opt else {
                        let _ = shutdown_tx.send(ShutdownSignal::Close {
                            code: DEFAULT_CLOSE_CODE,
                            reason: "client_disconnected".to_string(),
                        });
                        break;
                    };

                    let msg = match msg_result {
                        Ok(m) => m,
                        Err(e) => {
                            tracing::error!(
                                error.type = "ws_client_receive_error",
                                error = %e,
                                "client_receive_error: {}",
                                e
                            );
                            let _ = shutdown_tx.send(ShutdownSignal::Close {
                                code: DEFAULT_CLOSE_CODE,
                                reason: "client_error".to_string(),
                            });
                            break;
                        }
                    };

                    match msg {
                        Message::Text(text) => {
                            let text_owned = text.to_string();
                            let text_str = match first_msg_transformer.take() {
                                Some(t) => t(text_owned),
                                None => text_owned,
                            };

                            let text_str = match client_message_filter.as_ref() {
                                Some(filter) => match filter(text_str) {
                                    Some(s) => s,
                                    None => continue,
                                },
                                None => text_str,
                            };

                            let data = text_str.into_bytes();

                            if Self::process_data_message(&mut pending, data, true, &control_types, &shutdown_tx, &mut upstream_sender).await {
                                break;
                            }
                        }
                        Message::Binary(bytes) => {
                            // Note: Don't consume first_msg_transformer here - it's only meant for text messages.
                            // If the first message is binary, we preserve the transformer for any subsequent text message.
                            // This ensures authentication transformers are applied when a text message eventually arrives.
                            if first_msg_transformer.is_some() {
                                tracing::debug!("binary_message_received_before_text_transform");
                            }
                            let data = bytes.to_vec();

                            if Self::process_data_message(&mut pending, data, false, &control_types, &shutdown_tx, &mut upstream_sender).await {
                                break;
                            }
                        }
                        Message::Ping(data) => {
                            if let Err(e) = upstream_sender.send(TungsteniteMessage::Ping(data.to_vec().into())).await {
                                tracing::error!(
                                    error = ?e,
                                    "upstream_ping_failed"
                                );
                            }
                        }
                        Message::Pong(data) => {
                            if let Err(e) = upstream_sender.send(TungsteniteMessage::Pong(data.to_vec().into())).await {
                                tracing::error!(
                                    error = ?e,
                                    "upstream_pong_failed"
                                );
                            }
                        }
                        Message::Close(frame) => {
                            let (code, reason) = convert::extract_axum_close(frame, "client_closed");
                            tracing::info!(
                                hyprnote.ws.close.code = code,
                                hyprnote.ws.close.reason = %reason,
                                "ws_client_close_received"
                            );
                            let _ = shutdown_tx.send(ShutdownSignal::Close { code, reason });
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn run_upstream_to_client(
        mut upstream_receiver: UpstreamReceiver,
        mut client_sender: ClientSender,
        shutdown_tx: tokio::sync::broadcast::Sender<ShutdownSignal>,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<ShutdownSignal>,
        response_transformer: Option<ResponseTransformer>,
    ) {
        let mut pending_error: Option<(u16, String)> = None;

        loop {
            tokio::select! {
                biased;

                result = shutdown_rx.recv() => {
                    if let Ok(signal) = result {
                        if let ShutdownSignal::Close { code, reason } = signal {
                            let _ = client_sender.send(convert::to_axum_close(code, reason)).await;
                        }
                    }
                    break;
                }

                msg_opt = upstream_receiver.next() => {
                    let Some(msg_result) = msg_opt else {
                        let signal = pending_error
                            .take()
                            .map(|(code, reason)| ShutdownSignal::Close { code, reason })
                            .unwrap_or(ShutdownSignal::Abort);
                        let _ = shutdown_tx.send(signal);
                        break;
                    };

                    let msg = match msg_result {
                        Ok(m) => m,
                        Err(e) => {
                            tracing::error!(
                                error.type = "ws_upstream_receive_error",
                                error = %e,
                                "upstream_receive_error: {}",
                                e
                            );
                            let signal = pending_error
                                .take()
                                .map(|(code, reason)| ShutdownSignal::Close { code, reason })
                                .unwrap_or(ShutdownSignal::Abort);
                            let _ = shutdown_tx.send(signal);
                            break;
                        }
                    };

                    match msg {
                        TungsteniteMessage::Text(text) => {
                            let text_str = text.as_str();
                            let text_bytes = text_str.as_bytes();

                            if let Some(upstream_err) = Provider::detect_any_error(text_bytes) {
                                tracing::warn!(
                                    http.response.status_code = upstream_err.http_code,
                                    hyprnote.stt.provider.error_code = ?upstream_err.provider_code,
                                    error = %upstream_err.message,
                                    "upstream_error_detected"
                                );

                                pending_error = Some((
                                    upstream_err.to_ws_close_code(),
                                    upstream_err.message.clone(),
                                ));
                            }

                            let output_text = match &response_transformer {
                                Some(transformer) => match transformer(text_str) {
                                    Some(transformed) => transformed,
                                    None => continue,
                                },
                                None => text_str.to_string(),
                            };

                            if client_sender.send(Message::Text(output_text.into())).await.is_err() {
                                let _ = shutdown_tx.send(ShutdownSignal::Close {
                                    code: DEFAULT_CLOSE_CODE,
                                    reason: "client_send_failed".to_string(),
                                });
                                break;
                            }
                        }
                        TungsteniteMessage::Binary(data) => {
                            if client_sender.send(Message::Binary(data.to_vec().into())).await.is_err() {
                                let _ = shutdown_tx.send(ShutdownSignal::Close {
                                    code: DEFAULT_CLOSE_CODE,
                                    reason: "client_send_failed".to_string(),
                                });
                                break;
                            }
                        }
                        TungsteniteMessage::Ping(data) => {
                            if let Err(e) = client_sender.send(Message::Ping(data.to_vec().into())).await {
                                tracing::error!(
                                    error = ?e,
                                    "client_ping_failed"
                                );
                            }
                        }
                        TungsteniteMessage::Pong(data) => {
                            if let Err(e) = client_sender.send(Message::Pong(data.to_vec().into())).await {
                                tracing::error!(
                                    error = ?e,
                                    "client_pong_failed"
                                );
                            }
                        }
                        TungsteniteMessage::Close(frame) => {
                            let signal = if let Some((code, reason)) = pending_error.take() {
                                ShutdownSignal::Close { code, reason }
                            } else {
                                let (code, reason) =
                                    convert::extract_tungstenite_close(frame, "upstream_closed");
                                if code == 1000 {
                                    ShutdownSignal::Close { code, reason }
                                } else {
                                    tracing::warn!(
                                        hyprnote.ws.close.code = code,
                                        hyprnote.ws.close.reason = %reason,
                                        "ws_upstream_abnormal_close"
                                    );
                                    ShutdownSignal::Abort
                                }
                            };

                            if let ShutdownSignal::Close { code, reason } = &signal {
                                tracing::info!(
                                    hyprnote.ws.close.code = *code,
                                    hyprnote.ws.close.reason = %reason,
                                    "ws_upstream_close_received"
                                );
                            }

                            let _ = shutdown_tx.send(signal);
                            break;
                        }
                        TungsteniteMessage::Frame(_) => {}
                    }
                }
            }
        }
    }
}

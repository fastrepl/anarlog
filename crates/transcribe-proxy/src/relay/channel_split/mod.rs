mod coordinator;
mod io;
mod payload;

use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::Response;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use sentry::SentryFutureExt;
use tokio_tungstenite::tungstenite::ClientRequestBuilder;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::client::IntoClientRequest,
};

use owhisper_client::Provider;

use self::coordinator::{CoordinatorAction, SplitCoordinator, SplitEvent};
use self::io::{relay_client_to_upstreams, relay_upstream_to_events, send_rewritten, send_text};
use self::payload::{FinalizeMode, rewrite_split_response};
use super::types::{
    ClientMessageFilter, DEFAULT_CLOSE_CODE, InitialMessage, OnCloseCallback, ResponseTransformer,
    ShutdownSignal, convert,
};

fn proxy_debug_enabled() -> bool {
    std::env::var("LISTENER_DEBUG")
        .map(|value| !value.is_empty() && value != "0" && value != "false")
        .unwrap_or(false)
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
            .map_err(|error| crate::ProxyError::InvalidRequest(error.to_string()))?;
        hypr_observability::inject_current_trace_context(req.headers_mut());

        let result = tokio::time::timeout(timeout, connect_async(req)).await;
        match result {
            Ok(Ok((stream, _))) => Ok(stream),
            Ok(Err(error)) => Err(crate::ProxyError::ConnectionFailed(error.to_string())),
            Err(_) => Err(crate::ProxyError::ConnectionTimeout),
        }
    }

    pub async fn handle_upgrade(&self, ws: WebSocketUpgrade) -> Response<Body> {
        let proxy = self.clone();
        let hub = sentry::Hub::current();
        ws.on_upgrade(move |socket| {
            async move {
                if let Err(error) = proxy.handle(socket).await {
                    tracing::error!(error = %error, "channel_split_proxy_error");
                }
            }
            .bind_hub(sentry::Hub::new_from_top(hub))
        })
        .into_response()
    }

    async fn handle(
        &self,
        client_socket: axum::extract::ws::WebSocket,
    ) -> Result<(), crate::ProxyError> {
        tracing::info!("connecting_to_upstream(channel_split)");
        let (mic_upstream, spk_upstream) = tokio::try_join!(
            Self::connect_upstream(&self.mic_request, self.connect_timeout),
            Self::connect_upstream(&self.spk_request, self.connect_timeout),
        )?;

        let start_time = Instant::now();

        self.run_relay(client_socket, mic_upstream, spk_upstream)
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
        &self,
        client_socket: axum::extract::ws::WebSocket,
        mic_upstream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
        spk_upstream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    ) {
        let (mut mic_tx, mut mic_rx) = mic_upstream.split();
        let (mut spk_tx, mut spk_rx) = spk_upstream.split();
        let (mut client_tx, client_rx) = client_socket.split();

        if let Some(message) = &self.initial_message {
            let upstream_message =
                tokio_tungstenite::tungstenite::Message::Text(message.as_str().into());
            if mic_tx.send(upstream_message.clone()).await.is_err()
                || spk_tx.send(upstream_message).await.is_err()
            {
                tracing::error!("channel_split_initial_message_send_failed");
                return;
            }
        }

        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<ShutdownSignal>(1);
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<SplitEvent>(64);

        let client_to_upstreams = relay_client_to_upstreams(
            client_rx,
            mic_tx,
            spk_tx,
            self.client_message_filter.clone(),
            shutdown_tx.clone(),
            event_tx.clone(),
        );
        let mic_to_events =
            relay_upstream_to_events(&mut mic_rx, 0, event_tx.clone(), shutdown_tx.clone());
        let spk_to_events = relay_upstream_to_events(&mut spk_rx, 1, event_tx, shutdown_tx.clone());

        let event_coordinator = {
            let shutdown_tx = shutdown_tx.clone();
            let response_transformer = self.response_transformer.clone();
            async move {
                let mut coordinator = SplitCoordinator::default();

                while let Some(event) = event_rx.recv().await {
                    let actions = match event {
                        SplitEvent::FinalizeRequested => {
                            coordinator.handle_finalize_requested();
                            Vec::new()
                        }
                        SplitEvent::Text { channel, raw } => {
                            let raw_log = proxy_debug_enabled().then(|| raw.clone());
                            let upstream_error = Provider::detect_any_error(raw.as_bytes())
                                .map(|error| (error.to_ws_close_code(), error.message));
                            let transformed = match &response_transformer {
                                Some(transformer) => transformer(&raw),
                                None => Some(raw),
                            };
                            let transformed_log = proxy_debug_enabled()
                                .then(|| transformed.as_deref().unwrap_or("<none>").to_string());

                            let rewritten = transformed.as_deref().and_then(|text| {
                                rewrite_split_response(
                                    text,
                                    channel as i32,
                                    2,
                                    FinalizeMode::Preserve,
                                )
                            });
                            let passthrough_text = match (transformed, rewritten.as_ref()) {
                                (Some(text), None) => Some(text),
                                _ => None,
                            };

                            if proxy_debug_enabled() {
                                let rewritten_log = rewritten
                                    .as_ref()
                                    .and_then(|_| transformed_log.as_deref())
                                    .and_then(|text| {
                                        rewrite_split_response(
                                            text,
                                            channel as i32,
                                            2,
                                            FinalizeMode::Preserve,
                                        )
                                    })
                                    .and_then(|response| response.into_text())
                                    .unwrap_or_else(|| "<none>".to_string());
                                let passthrough_log =
                                    passthrough_text.as_deref().unwrap_or("<none>");

                                tracing::info!(
                                    hyprnote.stream.channel = channel,
                                    raw = %raw_log.as_deref().unwrap_or("<none>"),
                                    transformed = %transformed_log.as_deref().unwrap_or("<none>"),
                                    rewritten = %rewritten_log,
                                    passthrough = %passthrough_log,
                                    "channel_split_transformed_text"
                                );
                            }

                            coordinator.handle_text(
                                channel,
                                rewritten,
                                passthrough_text,
                                upstream_error,
                            )
                        }
                        SplitEvent::UpstreamClosed {
                            channel,
                            code,
                            reason,
                        } => coordinator.handle_upstream_closed(channel, code, reason),
                        SplitEvent::Fatal(signal) => coordinator.handle_fatal(signal),
                        SplitEvent::ClientClosed => coordinator
                            .handle_client_closed(DEFAULT_CLOSE_CODE, "client_closed".to_string()),
                    };

                    let mut should_break = false;
                    for action in actions {
                        match action {
                            CoordinatorAction::ForwardText(text) => {
                                if !send_text(&mut client_tx, text).await {
                                    let _ = shutdown_tx.send(ShutdownSignal::Close {
                                        code: DEFAULT_CLOSE_CODE,
                                        reason: "client_send_failed".to_string(),
                                    });
                                    should_break = true;
                                    break;
                                }
                            }
                            CoordinatorAction::ForwardRewritten(response) => {
                                if !send_rewritten(&mut client_tx, response).await {
                                    let _ = shutdown_tx.send(ShutdownSignal::Close {
                                        code: DEFAULT_CLOSE_CODE,
                                        reason: "client_send_failed".to_string(),
                                    });
                                    should_break = true;
                                    break;
                                }
                            }
                            CoordinatorAction::CloseDownstream { code, reason } => {
                                let _ = client_tx.send(convert::to_axum_close(code, reason)).await;
                            }
                            CoordinatorAction::AbortDownstream => {
                                should_break = true;
                            }
                            CoordinatorAction::ShutdownUpstreams(signal) => {
                                let _ = shutdown_tx.send(signal);
                                should_break = true;
                                break;
                            }
                        }
                    }

                    if should_break {
                        break;
                    }
                }
            }
        };

        tokio::join!(
            client_to_upstreams,
            mic_to_events,
            spk_to_events,
            event_coordinator,
        );
    }
}

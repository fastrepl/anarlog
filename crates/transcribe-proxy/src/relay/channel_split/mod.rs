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
use self::io::{
    GatedUpstream, relay_client_to_upstreams, relay_upstream_to_events, send_rewritten, send_text,
};
use self::payload::{FinalizeMode, rewrite_split_response};
use super::types::{
    ClientBinaryMessageMapper, ClientMessageFilter, DEFAULT_CLOSE_CODE, InitialMessage,
    OnCloseCallback, ResponseTransformer, ShutdownSignal, UpstreamReadiness, convert,
    ready_channel,
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
    initial_messages: [Option<InitialMessage>; 2],
    response_transformers: [Option<ResponseTransformer>; 2],
    connect_timeout: Duration,
    on_close: Option<OnCloseCallback>,
    client_message_filters: [Option<ClientMessageFilter>; 2],
    client_binary_message_mapper: Option<ClientBinaryMessageMapper>,
    upstream_readiness: Option<UpstreamReadiness>,
}

impl ChannelSplitProxy {
    pub fn with_split_requests(
        mic_request: ClientRequestBuilder,
        spk_request: ClientRequestBuilder,
        initial_messages: [Option<InitialMessage>; 2],
        response_transformers: [Option<ResponseTransformer>; 2],
        connect_timeout: Duration,
        on_close: Option<OnCloseCallback>,
    ) -> Self {
        Self {
            mic_request,
            spk_request,
            initial_messages,
            response_transformers,
            connect_timeout,
            on_close,
            client_message_filters: [None, None],
            client_binary_message_mapper: None,
            upstream_readiness: None,
        }
    }

    pub fn with_client_message_filters(
        mut self,
        filters: [Option<ClientMessageFilter>; 2],
    ) -> Self {
        self.client_message_filters = filters;
        self
    }

    pub fn with_upstream_readiness(mut self, readiness: UpstreamReadiness) -> Self {
        self.upstream_readiness = Some(readiness);
        self
    }

    pub fn with_client_binary_message_mapper(mut self, mapper: ClientBinaryMessageMapper) -> Self {
        self.client_binary_message_mapper = Some(mapper);
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
        anlg_observability::inject_current_trace_context(req.headers_mut());

        let result = tokio::time::timeout(timeout, connect_async(req)).await;
        match result {
            Ok(Ok((stream, _))) => Ok(stream),
            Ok(Err(error)) => Err(crate::ProxyError::ConnectionFailed(error.to_string())),
            Err(_) => Err(crate::ProxyError::ConnectionTimeout),
        }
    }

    pub async fn handle_upgrade_with_guard<G: Send + 'static>(
        &self,
        ws: WebSocketUpgrade,
        guard: G,
    ) -> Response<Body> {
        let proxy = self.clone();
        let hub = sentry::Hub::current();
        ws.on_upgrade(move |socket| {
            async move {
                let _guard = guard;
                if let Err(_error) = proxy.handle(socket).await {
                    tracing::error!(error.type = "channel_split_proxy_error", "channel_split_proxy_error");
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
            anarlog.duration_ms = %(duration.as_millis() as u64),
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

        for (tx, message) in [
            (&mut mic_tx, &self.initial_messages[0]),
            (&mut spk_tx, &self.initial_messages[1]),
        ] {
            if let Some(message) = message
                && tx
                    .send(tokio_tungstenite::tungstenite::Message::Text(
                        message.as_str().into(),
                    ))
                    .await
                    .is_err()
            {
                tracing::error!("channel_split_initial_message_send_failed");
                return;
            }
        }

        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<ShutdownSignal>(1);
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<SplitEvent>(64);

        let readiness = self.upstream_readiness.as_ref();
        let (mic_ready_tx, mic_ready_rx) = ready_channel(readiness);
        let (spk_ready_tx, spk_ready_rx) = ready_channel(readiness);
        let mic_readiness = readiness.map(|r| (r.clone(), mic_ready_tx));
        let spk_readiness = readiness.map(|r| (r.clone(), spk_ready_tx));

        let client_to_upstreams = relay_client_to_upstreams(
            client_rx,
            GatedUpstream::new(mic_tx, mic_ready_rx),
            GatedUpstream::new(spk_tx, spk_ready_rx),
            self.client_message_filters.clone(),
            self.client_binary_message_mapper.clone(),
            shutdown_tx.clone(),
            event_tx.clone(),
        );
        let mic_to_events = relay_upstream_to_events(
            &mut mic_rx,
            0,
            event_tx.clone(),
            shutdown_tx.clone(),
            mic_readiness,
        );
        let spk_to_events =
            relay_upstream_to_events(&mut spk_rx, 1, event_tx, shutdown_tx.clone(), spk_readiness);

        let event_coordinator = {
            let shutdown_tx = shutdown_tx.clone();
            let response_transformers = self.response_transformers.clone();
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
                            let transformed = match &response_transformers[channel as usize] {
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
                                    .and(transformed_log.as_deref())
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
                                    anarlog.stream.channel = channel,
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

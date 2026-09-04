mod anarlog;
mod passthrough;
mod session;

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::{
    extract::{FromRequestParts, State, WebSocketUpgrade},
    http::{StatusCode, header::RETRY_AFTER, request::Parts},
    response::{IntoResponse, Response},
};
use owhisper_client::Provider;

use crate::anarlog_routing::should_use_anarlog_routing;
use crate::query_params::{QueryParams, QueryValue};
use crate::relay::OnCloseCallback;
use crate::session_gate::SessionPermit;

use super::AppState;

use anlg_analytics::{AuthenticatedUserId, DeviceFingerprint};

pub enum ProxyBuildError {
    SessionInitFailed(String),
    ProxyError(crate::ProxyError),
}

impl From<crate::ProxyError> for ProxyBuildError {
    fn from(error: crate::ProxyError) -> Self {
        Self::ProxyError(error)
    }
}

pub fn parse_param<T: std::str::FromStr>(params: &QueryParams, key: &str, default: T) -> T {
    params
        .get_first(key)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

pub struct AnalyticsContext {
    pub fingerprint: Option<String>,
    pub user_id: Option<String>,
}

pub struct DrainPermit(SessionPermit);

impl FromRequestParts<AppState> for DrainPermit {
    type Rejection = Response;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match state.session_gate.try_acquire() {
            Ok(permit) => Ok(Self(permit)),
            Err(_) => {
                tracing::info!("stt_ws_rejected_draining");
                Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    [(RETRY_AFTER, "2")],
                    "server is draining existing sessions",
                )
                    .into_response())
            }
        }
    }
}

pub fn build_on_close_callback(
    config: &crate::config::SttProxyConfig,
    provider: Provider,
    analytics_ctx: &AnalyticsContext,
) -> Option<OnCloseCallback> {
    let analytics = config.analytics.as_ref()?.clone();
    let provider_name = format!("{:?}", provider).to_lowercase();
    let fingerprint = analytics_ctx.fingerprint.clone();
    let user_id = analytics_ctx.user_id.clone();

    Some(Arc::new(move |duration| {
        let analytics = analytics.clone();
        let provider_name = provider_name.clone();
        let fingerprint = fingerprint.clone();
        let user_id = user_id.clone();
        Box::pin(async move {
            analytics
                .report_stt(crate::analytics::SttEvent {
                    fingerprint,
                    user_id,
                    provider: provider_name,
                    duration,
                })
                .await;
        }) as Pin<Box<dyn Future<Output = ()> + Send>>
    }))
}

impl<S> FromRequestParts<S> for AnalyticsContext
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let fingerprint = parts
            .extensions
            .get::<DeviceFingerprint>()
            .map(|id| id.0.clone());
        let user_id = parts
            .extensions
            .get::<AuthenticatedUserId>()
            .map(|id| id.0.clone());
        Ok(AnalyticsContext {
            fingerprint,
            user_id,
        })
    }
}

#[tracing::instrument(
    name = "stt.ws.upgrade",
    skip(state, permit, analytics_ctx, ws, params),
    fields(
        anarlog.subsystem = "stt",
        http.response.status_code = tracing::field::Empty,
        anarlog.stt.provider.name = tracing::field::Empty,
        anarlog.stt.routing_strategy = tracing::field::Empty,
        anarlog.stt.model = tracing::field::Empty,
        anarlog.stt.language_codes = tracing::field::Empty,
        anarlog.audio.sample_rate_hz = tracing::field::Empty,
        anarlog.audio.channel_count = tracing::field::Empty,
        error.type = tracing::field::Empty,
        otel.status_code = tracing::field::Empty
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    DrainPermit(permit): DrainPermit,
    analytics_ctx: AnalyticsContext,
    ws: WebSocketUpgrade,
    mut params: QueryParams,
) -> Response {
    let span = tracing::Span::current();
    span.record("anarlog.subsystem", "stt");

    let is_anarlog_routing = should_use_anarlog_routing(params.get_first("provider"));

    let selected = match state.resolve_provider(&mut params) {
        Ok(v) => v,
        Err(resp) => {
            span.record("http.response.status_code", resp.status().as_u16() as i64);
            anlg_observability::mark_span_as_error(&span, "provider_selection_failed");
            tracing::warn!(
                parent: &span,
                error.type = "provider_selection_failed",
                "stt_provider_selection_failed"
            );
            return resp;
        }
    };

    let provider = selected.provider();
    let provider_name = format!("{:?}", provider).to_lowercase();
    let model = params.get_first("model").unwrap_or("default");
    let sample_rate: u32 = parse_param(&params, "sample_rate", 16000);
    let channels: u8 = parse_param(&params, "channels", 1);
    let languages = params.get_languages();
    let languages_str = languages
        .iter()
        .map(|l| l.iso639().to_string())
        .collect::<Vec<_>>()
        .join(",");

    span.record("anarlog.stt.provider.name", provider_name.as_str());
    span.record(
        "anarlog.stt.routing_strategy",
        if is_anarlog_routing {
            "anarlog"
        } else {
            "direct"
        },
    );
    span.record("anarlog.stt.model", model);
    span.record("anarlog.audio.sample_rate_hz", sample_rate);
    span.record("anarlog.audio.channel_count", channels as i64);
    if !languages_str.is_empty() {
        span.record("anarlog.stt.language_codes", languages_str.as_str());
    }

    tracing::info!(
        parent: &span,
        anarlog.stt.provider.name = %provider_name,
        anarlog.stt.routing_strategy = %(if is_anarlog_routing { "anarlog" } else { "direct" }),
        anarlog.stt.model = %model,
        anarlog.audio.sample_rate_hz = sample_rate,
        anarlog.audio.channel_count = channels,
        "stt_ws_session_started"
    );

    sentry::configure_scope(|scope| {
        scope.set_tag("anarlog.stt.provider.name", &provider_name);
        scope.set_tag(
            "anarlog.stt.routing_strategy",
            if is_anarlog_routing {
                "anarlog"
            } else {
                "direct"
            },
        );

        scope.set_tag("anarlog.stt.model", model);
        let languages: Vec<_> = languages.iter().map(|l| l.iso639().to_string()).collect();
        if !languages.is_empty() {
            scope.set_tag("anarlog.stt.language_codes", languages.join(","));
        }

        let keywords = params
            .get("keyword")
            .or_else(|| params.get("keywords"))
            .map(|v| match v {
                QueryValue::Single(s) => s.split(',').count(),
                QueryValue::Multi(vec) => vec.len(),
            })
            .unwrap_or(0);

        let mut ctx = BTreeMap::new();
        ctx.insert("anarlog.audio.sample_rate_hz".into(), sample_rate.into());
        ctx.insert("anarlog.audio.channel_count".into(), channels.into());
        ctx.insert("anarlog.stt.keyword_count".into(), keywords.into());
        ctx.insert("anarlog.stt.language_count".into(), languages.len().into());
        scope.set_context("anarlog.stt.request", sentry::protocol::Context::Other(ctx));
    });

    let proxy_result = if is_anarlog_routing {
        anarlog::build_proxy(&state, &selected, &params, analytics_ctx).await
    } else {
        passthrough::build_proxy(&state, &selected, &params, analytics_ctx)
            .await
            .map(crate::relay::StreamingProxy::single)
    };

    let proxy = match proxy_result {
        Ok(p) => p,
        Err(ProxyBuildError::SessionInitFailed(_e)) => {
            span.record(
                "http.response.status_code",
                StatusCode::BAD_GATEWAY.as_u16() as i64,
            );
            anlg_observability::mark_span_as_error(&span, "session_init_failed");
            tracing::error!(
                parent: &span,
                error.type = "session_init_failed",
                anarlog.stt.provider.name = ?selected.provider(),
                "session_init_failed"
            );
            sentry::configure_scope(|scope| {
                scope.set_tag("error.type", "session_init_failed");
            });
            return (StatusCode::BAD_GATEWAY, "provider session unavailable").into_response();
        }
        Err(ProxyBuildError::ProxyError(_e)) => {
            span.record(
                "http.response.status_code",
                StatusCode::BAD_REQUEST.as_u16() as i64,
            );
            return (StatusCode::BAD_REQUEST, "invalid provider request").into_response();
        }
    };

    proxy
        .handle_upgrade_with_guard(ws, permit)
        .await
        .into_response()
}

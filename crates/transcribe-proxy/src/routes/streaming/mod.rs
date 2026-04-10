mod char_stt;
mod passthrough;
mod session;

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::{
    extract::{FromRequestParts, State, WebSocketUpgrade},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use owhisper_client::Provider;

use crate::char_routing::should_use_char_routing;
use crate::query_params::{QueryParams, QueryValue};
use crate::relay::OnCloseCallback;

use super::AppState;

use hypr_analytics::{AuthenticatedUserId, DeviceFingerprint};

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
    skip(state, analytics_ctx, ws, params),
    fields(
        char.subsystem = "stt",
        http.response.status_code = tracing::field::Empty,
        char.stt.provider.name = tracing::field::Empty,
        char.stt.routing_strategy = tracing::field::Empty,
        char.stt.model = tracing::field::Empty,
        char.stt.language_codes = tracing::field::Empty,
        char.audio.sample_rate_hz = tracing::field::Empty,
        char.audio.channel_count = tracing::field::Empty,
        enduser.id = tracing::field::Empty,
        enduser.pseudo.id = tracing::field::Empty,
        error.type = tracing::field::Empty,
        otel.status_code = tracing::field::Empty
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    analytics_ctx: AnalyticsContext,
    ws: WebSocketUpgrade,
    mut params: QueryParams,
) -> Response {
    let span = tracing::Span::current();
    span.record("char.subsystem", "stt");

    let is_char_routing = should_use_char_routing(params.get_first("provider"));

    let selected = match state.resolve_provider(&mut params) {
        Ok(v) => v,
        Err(resp) => {
            span.record("http.response.status_code", resp.status().as_u16() as i64);
            hypr_observability::mark_span_as_error(&span, "provider_selection_failed");
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

    span.record("char.stt.provider.name", provider_name.as_str());
    span.record(
        "char.stt.routing_strategy",
        if is_char_routing {
            "char"
        } else {
            "direct"
        },
    );
    span.record("char.stt.model", model);
    span.record("char.audio.sample_rate_hz", sample_rate);
    span.record("char.audio.channel_count", channels as i64);
    if let Some(user_id) = analytics_ctx.user_id.as_deref() {
        span.record("enduser.id", user_id);
    }
    if let Some(fingerprint) = analytics_ctx.fingerprint.as_deref() {
        span.record("enduser.pseudo.id", fingerprint);
    }
    if !languages_str.is_empty() {
        span.record("char.stt.language_codes", languages_str.as_str());
    }

    tracing::info!(
        parent: &span,
        char.stt.provider.name = %provider_name,
        char.stt.routing_strategy = %(if is_char_routing { "char" } else { "direct" }),
        char.stt.model = %model,
        char.audio.sample_rate_hz = sample_rate,
        char.audio.channel_count = channels,
        "stt_ws_session_started"
    );

    sentry::configure_scope(|scope| {
        scope.set_tag("char.stt.provider.name", &provider_name);
        scope.set_tag(
            "char.stt.routing_strategy",
            if is_char_routing {
                "char"
            } else {
                "direct"
            },
        );

        scope.set_tag("char.stt.model", model);
        let languages: Vec<_> = languages.iter().map(|l| l.iso639().to_string()).collect();
        if !languages.is_empty() {
            scope.set_tag("char.stt.language_codes", languages.join(","));
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
        ctx.insert("char.audio.sample_rate_hz".into(), sample_rate.into());
        ctx.insert("char.audio.channel_count".into(), channels.into());
        ctx.insert("char.stt.keyword_count".into(), keywords.into());
        ctx.insert("char.stt.language_count".into(), languages.len().into());
        scope.set_context(
            "char.stt.request",
            sentry::protocol::Context::Other(ctx),
        );
    });

    let proxy_result = if is_char_routing {
        char_stt::build_proxy(&state, &selected, &params, analytics_ctx).await
    } else {
        passthrough::build_proxy(&state, &selected, &params, analytics_ctx)
            .await
            .map(crate::relay::StreamingProxy::single)
    };

    let proxy = match proxy_result {
        Ok(p) => p,
        Err(ProxyBuildError::SessionInitFailed(e)) => {
            span.record(
                "http.response.status_code",
                StatusCode::BAD_GATEWAY.as_u16() as i64,
            );
            hypr_observability::mark_span_as_error(&span, "session_init_failed");
            tracing::error!(
                parent: &span,
                error.type = "session_init_failed",
                error = %e,
                char.stt.provider.name = ?selected.provider(),
                "session_init_failed"
            );
            sentry::configure_scope(|scope| {
                scope.set_tag("error.type", "session_init_failed");
            });
            return (StatusCode::BAD_GATEWAY, e).into_response();
        }
        Err(ProxyBuildError::ProxyError(e)) => {
            span.record(
                "http.response.status_code",
                StatusCode::BAD_REQUEST.as_u16() as i64,
            );
            hypr_observability::mark_span_as_error(&span, "proxy_build_failed");
            tracing::error!(
                parent: &span,
                error.type = "proxy_build_failed",
                error = %e,
                char.stt.provider.name = ?provider,
                "proxy_build_failed"
            );
            sentry::configure_scope(|scope| {
                scope.set_tag("error.type", "proxy_build_failed");
            });
            return (StatusCode::BAD_REQUEST, format!("{}", e)).into_response();
        }
    };

    proxy.handle_upgrade(ws).await.into_response()
}

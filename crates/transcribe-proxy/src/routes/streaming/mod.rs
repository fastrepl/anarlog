mod common;
mod hyprnote;
mod passthrough;
mod session;

use std::collections::BTreeMap;

use axum::{
    extract::{FromRequestParts, State, WebSocketUpgrade},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};

use crate::hyprnote_routing::should_use_hyprnote_routing;
use crate::query_params::{QueryParams, QueryValue};

use super::AppState;
use common::{ProxyBuildError, parse_param};

use hypr_analytics::{AuthenticatedUserId, DeviceFingerprint};

pub struct AnalyticsContext {
    pub fingerprint: Option<String>,
    pub user_id: Option<String>,
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
        hyprnote.subsystem = "stt",
        hyprnote.stt.provider.name = tracing::field::Empty,
        hyprnote.stt.routing_strategy = tracing::field::Empty,
        hyprnote.stt.model = tracing::field::Empty,
        hyprnote.stt.language_codes = tracing::field::Empty,
        hyprnote.audio.sample_rate_hz = tracing::field::Empty,
        hyprnote.audio.channel_count = tracing::field::Empty,
        enduser.id = tracing::field::Empty,
        enduser.pseudo.id = tracing::field::Empty
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    analytics_ctx: AnalyticsContext,
    ws: WebSocketUpgrade,
    mut params: QueryParams,
) -> Response {
    let span = tracing::Span::current();
    span.record("hyprnote.subsystem", "stt");

    let is_hyprnote_routing = should_use_hyprnote_routing(params.get_first("provider"));

    let selected = match state.resolve_provider(&mut params) {
        Ok(v) => v,
        Err(resp) => {
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

    span.record("hyprnote.stt.provider.name", provider_name.as_str());
    span.record(
        "hyprnote.stt.routing_strategy",
        if is_hyprnote_routing {
            "hyprnote"
        } else {
            "direct"
        },
    );
    span.record("hyprnote.stt.model", model);
    span.record("hyprnote.audio.sample_rate_hz", sample_rate);
    span.record("hyprnote.audio.channel_count", channels as i64);
    if let Some(user_id) = analytics_ctx.user_id.as_deref() {
        span.record("enduser.id", user_id);
    }
    if let Some(fingerprint) = analytics_ctx.fingerprint.as_deref() {
        span.record("enduser.pseudo.id", fingerprint);
    }
    if !languages_str.is_empty() {
        span.record("hyprnote.stt.language_codes", languages_str.as_str());
    }

    tracing::info!(
        parent: &span,
        hyprnote.stt.provider.name = %provider_name,
        hyprnote.stt.routing_strategy = %(if is_hyprnote_routing { "hyprnote" } else { "direct" }),
        hyprnote.stt.model = %model,
        hyprnote.audio.sample_rate_hz = sample_rate,
        hyprnote.audio.channel_count = channels,
        "stt_ws_session_started"
    );

    sentry::configure_scope(|scope| {
        scope.set_tag("hyprnote.stt.provider.name", &provider_name);
        scope.set_tag(
            "hyprnote.stt.routing_strategy",
            if is_hyprnote_routing {
                "hyprnote"
            } else {
                "direct"
            },
        );

        scope.set_tag("hyprnote.stt.model", model);
        let languages: Vec<_> = languages.iter().map(|l| l.iso639().to_string()).collect();
        if !languages.is_empty() {
            scope.set_tag("hyprnote.stt.language_codes", languages.join(","));
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
        ctx.insert("hyprnote.audio.sample_rate_hz".into(), sample_rate.into());
        ctx.insert("hyprnote.audio.channel_count".into(), channels.into());
        ctx.insert("hyprnote.stt.keyword_count".into(), keywords.into());
        ctx.insert("hyprnote.stt.language_count".into(), languages.len().into());
        scope.set_context(
            "hyprnote.stt.request",
            sentry::protocol::Context::Other(ctx),
        );
    });

    let proxy_result = if is_hyprnote_routing {
        hyprnote::build_proxy(&state, &selected, &params, analytics_ctx).await
    } else {
        passthrough::build_proxy(&state, &selected, &params, analytics_ctx)
            .await
            .map(hyprnote::StreamingProxy::Single)
    };

    let proxy = match proxy_result {
        Ok(p) => p,
        Err(ProxyBuildError::SessionInitFailed(e)) => {
            tracing::error!(
                parent: &span,
                error.type = "session_init_failed",
                error.message = %e,
                hyprnote.stt.provider.name = ?selected.provider(),
                "session_init_failed"
            );
            sentry::configure_scope(|scope| {
                scope.set_tag("error.type", "session_init_failed");
            });
            return (StatusCode::BAD_GATEWAY, e).into_response();
        }
        Err(ProxyBuildError::ProxyError(e)) => {
            tracing::error!(
                parent: &span,
                error.type = "proxy_build_failed",
                error.message = %e,
                hyprnote.stt.provider.name = ?provider,
                "proxy_build_failed"
            );
            sentry::configure_scope(|scope| {
                scope.set_tag("error.type", "proxy_build_failed");
            });
            return (StatusCode::BAD_REQUEST, format!("{}", e)).into_response();
        }
    };

    match proxy {
        hyprnote::StreamingProxy::Single(p) => p.handle_upgrade(ws).await.into_response(),
        hyprnote::StreamingProxy::ChannelSplit(p) => p.handle_upgrade(ws).await.into_response(),
    }
}

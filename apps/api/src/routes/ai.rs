use crate::{PAID_ENTITLEMENTS, rate_limit};
use crate::{
    auth::{self, AuthState},
    env::RuntimeConfig,
};
use axum::{Router, middleware};
use std::{num::NonZeroU32, sync::Arc, time::Duration};

pub(crate) fn router(
    env: &RuntimeConfig,
    session_gate: anlg_transcribe_proxy::SessionGate,
    analytics: Arc<anlg_analytics::AnalyticsClient>,
) -> Router {
    let llm_config =
        anlg_llm_proxy::LlmProxyConfig::new(env.llm.as_ref().expect("AI configuration resolved"))
            .with_analytics(analytics.clone());
    let stt_config = anlg_transcribe_proxy::SttProxyConfig::new(
        env.stt.as_ref().expect("AI configuration resolved"),
        &env.supabase,
    )
    .with_anarlog_routing(anlg_transcribe_proxy::AnarlogRoutingConfig::default())
    .with_analytics(analytics.clone());

    let stt_rate_limit = rate_limit::RateLimitState::builder()
        .pro(
            governor::Quota::with_period(Duration::from_mins(5))
                .unwrap()
                .allow_burst(NonZeroU32::new(20).unwrap()),
        )
        .free(
            governor::Quota::with_period(Duration::from_hours(24))
                .unwrap()
                .allow_burst(NonZeroU32::new(3).unwrap()),
        )
        .build();
    let llm_rate_limit = rate_limit::RateLimitState::builder()
        .pro(
            governor::Quota::with_period(Duration::from_secs(1))
                .unwrap()
                .allow_burst(NonZeroU32::new(30).unwrap()),
        )
        .free(
            governor::Quota::with_period(Duration::from_hours(12))
                .unwrap()
                .allow_burst(NonZeroU32::new(5).unwrap()),
        )
        .build();
    let auth_state = AuthState::new(&env.supabase.supabase_url);
    let auth_state_paid = auth_state.clone().with_required_entitlements(
        PAID_ENTITLEMENTS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    );
    let research_config = env.research.clone();
    let pyannote_config = env
        .pyannote
        .as_ref()
        .map(anlg_api_pyannote::PyannoteConfig::new);
    let callbacks = Router::new().nest(
        "/stt",
        anlg_transcribe_proxy::callback_router(stt_config.clone()),
    );
    let paid_routes = if research_config.is_none() && pyannote_config.is_none() {
        Router::new()
    } else {
        let mut routes = Router::new();
        if let Some(config) = research_config {
            routes = routes.merge(anlg_api_research::router(config));
        }
        if let Some(config) = pyannote_config {
            routes = routes.nest("/pyannote", anlg_api_pyannote::router(config));
        }
        routes
            .route_layer(middleware::from_fn(auth::sentry_and_analytics))
            .route_layer(middleware::from_fn_with_state(
                auth_state_paid.clone(),
                auth::require_auth,
            ))
    };

    let stt_routes = Router::new()
        .merge(anlg_transcribe_proxy::listen_router_with_session_gate(
            stt_config.clone(),
            session_gate.clone(),
        ))
        .nest(
            "/stt",
            anlg_transcribe_proxy::router_with_session_gate(stt_config, session_gate.clone()),
        )
        .route_layer(middleware::from_fn_with_state(
            stt_rate_limit,
            rate_limit::rate_limit,
        ));

    let llm_routes = Router::new()
        .merge(anlg_llm_proxy::chat_completions_router(llm_config.clone()))
        .nest("/llm", anlg_llm_proxy::router(llm_config))
        .route_layer(middleware::from_fn_with_state(
            llm_rate_limit,
            rate_limit::rate_limit,
        ));

    let authenticated = stt_routes
        .merge(llm_routes)
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state,
            auth::require_auth,
        ));
    authenticated.merge(paid_routes).merge(callbacks)
}

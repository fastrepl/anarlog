pub mod batch;
pub mod callback;
mod error;
mod model_resolution;
pub mod status;
pub mod streaming;

use std::sync::{Arc, LazyLock};

use axum::{
    Router,
    extract::{DefaultBodyLimit, FromRequestParts},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use owhisper_client::Provider;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::anarlog_routing::{AnarlogRouter, RoutingMode, should_use_anarlog_routing};
use crate::config::SttProxyConfig;
use crate::provider_selector::{ProviderSelector, SelectedProvider};
use crate::query_params::QueryParams;
use crate::session_gate::SessionGate;
use crate::supabase::SupabaseClient;

pub(crate) use error::{RouteError, parse_async_provider};

const MAX_BATCH_AUDIO_BODY_BYTES: usize = 512 * 1024 * 1024;
const MAX_BATCH_CALLBACK_BODY_BYTES: usize = 64 * 1024;
const MAX_CONCURRENT_BATCH_REQUESTS: usize = 4;

static BATCH_REQUEST_SLOTS: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(MAX_CONCURRENT_BATCH_REQUESTS)));

#[derive(Clone)]
pub(crate) struct AppState {
    pub config: SttProxyConfig,
    pub selector: ProviderSelector,
    pub router: Option<Arc<AnarlogRouter>>,
    pub client: reqwest::Client,
    pub session_gate: SessionGate,
    batch_requests: Arc<Semaphore>,
}

impl FromRequestParts<AppState> for SupabaseClient {
    type Rejection = RouteError;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let url = state
            .config
            .supabase
            .url
            .as_deref()
            .ok_or(RouteError::MissingConfig("supabase_url not configured"))?;
        let key =
            state
                .config
                .supabase
                .service_role_key
                .as_deref()
                .ok_or(RouteError::MissingConfig(
                    "supabase_service_role_key not configured",
                ))?;
        Ok(Self::new(state.client.clone(), url, key))
    }
}

impl AppState {
    pub fn try_acquire_batch_slot(&self) -> Result<OwnedSemaphorePermit, RouteError> {
        self.batch_requests
            .clone()
            .try_acquire_owned()
            .map_err(|_| RouteError::TooManyRequests("too many concurrent batch requests"))
    }

    #[allow(clippy::result_large_err)]
    pub fn resolve_provider(&self, params: &mut QueryParams) -> Result<SelectedProvider, Response> {
        let provider_param = params.remove_first("provider");

        if should_use_anarlog_routing(provider_param.as_deref()) {
            return self.resolve_anarlog_provider(params);
        }

        let requested = match provider_param {
            Some(s) => match s.parse::<Provider>() {
                Ok(p) => Some(p),
                Err(_) => {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!("Invalid provider: {}. Supported providers: anarlog, deepgram, soniox, assemblyai, gladia, elevenlabs, fireworks, openai, mistral, dashscope", s)
                    ).into_response());
                }
            },
            None => None,
        };

        self.selector.select(requested).map_err(|e| {
            tracing::warn!(
                error = %e,
                anarlog.stt.requested_provider = ?requested,
                "provider_selection_failed"
            );
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        })
    }

    #[allow(clippy::result_large_err)]
    fn resolve_anarlog_provider(&self, params: &QueryParams) -> Result<SelectedProvider, Response> {
        let router = self.router.as_ref().ok_or_else(|| {
            tracing::warn!("anarlog_routing_not_configured");
            (StatusCode::BAD_REQUEST, "anarlog routing is not configured").into_response()
        })?;

        let languages = params.get_languages();
        let available_providers = self.selector.available_providers();
        let routed_provider = router.select_provider(&languages, &available_providers);

        tracing::debug!(
            anarlog.stt.language_codes = ?languages,
            anarlog.stt.available_providers = ?available_providers,
            anarlog.stt.provider.name = ?routed_provider,
            "anarlog_routing"
        );

        self.selector.select(routed_provider).map_err(|e| {
            tracing::warn!(
                error = %e,
                anarlog.stt.language_codes = ?languages,
                "anarlog_routing_failed"
            );
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        })
    }

    pub fn resolve_anarlog_provider_chain_for_mode(
        &self,
        mode: RoutingMode,
        params: &QueryParams,
    ) -> Vec<SelectedProvider> {
        let Some(router) = self.router.as_ref() else {
            return vec![];
        };

        let languages = params.get_languages();
        let available_providers = self.selector.available_providers();

        router
            .select_provider_chain_with_mode(mode, &languages, &available_providers)
            .into_iter()
            .filter_map(|p| self.selector.select(Some(p)).ok())
            .collect()
    }
}

fn make_state(config: SttProxyConfig, session_gate: SessionGate) -> AppState {
    let selector = config.provider_selector();
    let router = config.anarlog_router().map(Arc::new);

    AppState {
        config,
        selector,
        router,
        client: reqwest::Client::new(),
        session_gate,
        batch_requests: BATCH_REQUEST_SLOTS.clone(),
    }
}

fn with_common_layers(router: Router) -> Router {
    router.layer(DefaultBodyLimit::max(MAX_BATCH_AUDIO_BODY_BYTES))
}

pub fn router(config: SttProxyConfig) -> Router {
    router_with_session_gate(config, SessionGate::new())
}

pub fn router_with_session_gate(config: SttProxyConfig, session_gate: SessionGate) -> Router {
    let state = make_state(config, session_gate);

    with_common_layers(
        Router::new()
            .route("/", get(streaming::handler))
            .route("/", post(batch::handler))
            .route("/listen", get(streaming::handler))
            .route("/listen", post(batch::handler))
            .route("/status/{pipeline_id}", get(status::handler))
            .with_state(state),
    )
}

pub fn listen_router(config: SttProxyConfig) -> Router {
    listen_router_with_session_gate(config, SessionGate::new())
}

pub fn listen_router_with_session_gate(
    config: SttProxyConfig,
    session_gate: SessionGate,
) -> Router {
    let state = make_state(config, session_gate);

    with_common_layers(
        Router::new()
            .route("/listen", get(streaming::handler))
            .route("/listen", post(batch::handler))
            .with_state(state),
    )
}

pub fn callback_router(config: SttProxyConfig) -> Router {
    let state = make_state(config, SessionGate::new());

    Router::new()
        .route("/callback/{provider}/{id}", post(callback::handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::Env;

    fn test_state() -> AppState {
        let mut env = Env::default();
        env.stt.deepgram_api_key = Some("deepgram-key".to_string());

        let supabase = anlg_api_env::SupabaseEnv {
            supabase_url: String::new(),
            supabase_anon_key: String::new(),
            supabase_service_role_key: String::new(),
        };

        make_state(SttProxyConfig::new(&env, &supabase), SessionGate::new())
    }

    #[test]
    fn resolve_provider_defaults_when_query_param_is_missing() {
        let state = test_state();
        let mut params = QueryParams::default();

        let selected = state.resolve_provider(&mut params).unwrap();

        assert_eq!(selected.provider(), Provider::Deepgram);
    }

    #[tokio::test]
    async fn draining_rejects_new_streaming_sessions() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;

        let mut env = Env::default();
        env.stt.deepgram_api_key = Some("deepgram-key".to_string());
        let supabase = anlg_api_env::SupabaseEnv {
            supabase_url: String::new(),
            supabase_anon_key: String::new(),
            supabase_service_role_key: String::new(),
        };
        let gate = SessionGate::new();
        gate.begin_drain();
        let app = listen_router_with_session_gate(SttProxyConfig::new(&env, &supabase), gate);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/listen")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers().get("retry-after").unwrap(), "2");
    }

    #[test]
    fn batch_admission_is_bounded_and_shared_across_router_states() {
        let first = test_state();
        let second = test_state();
        let permits: Vec<_> = (0..MAX_CONCURRENT_BATCH_REQUESTS)
            .map(|_| first.try_acquire_batch_slot().unwrap())
            .collect();

        assert!(first.try_acquire_batch_slot().is_err());
        assert!(second.try_acquire_batch_slot().is_err());
        drop(permits);
        assert!(second.try_acquire_batch_slot().is_ok());
    }
}

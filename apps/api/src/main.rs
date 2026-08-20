mod auth;
mod env;
mod observability;
mod openapi;
mod rate_limit;

use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use axum::{Router, body::Body, extract::MatchedPath, http::HeaderMap, http::Request, middleware};
use sentry::integrations::tower::{NewSentryLayer, SentryHttpLayer};
use sentry::protocol::{Context, Value};
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tower_http::{
    classify::ServerErrorsFailureClass,
    cors::{self, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

use auth::AuthState;
use env::env;

use crate::env::Env;

const PAID_ENTITLEMENTS: &[&str] = &["hyprnote_pro", "hyprnote_lite"];

pub const DEVICE_FINGERPRINT_HEADER: &str = "x-device-fingerprint";
pub const REQUEST_ID_HEADER: &str = "x-request-id";

fn forwarded_header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn request_scheme(request: &Request<Body>) -> String {
    forwarded_header_value(request.headers(), "x-forwarded-proto")
        .or_else(|| request.uri().scheme_str().map(ToString::to_string))
        .unwrap_or_else(|| "http".to_string())
}

fn request_server_endpoint(request: &Request<Body>, scheme: &str) -> (Option<String>, Option<u16>) {
    let authority = forwarded_header_value(request.headers(), "x-forwarded-host")
        .or_else(|| {
            request
                .headers()
                .get("host")
                .and_then(|value| value.to_str().ok())
                .map(ToString::to_string)
        })
        .or_else(|| request.uri().host().map(ToString::to_string));
    let Some(authority) = authority else {
        return (None, None);
    };
    let authority = authority.trim();
    if authority.is_empty() {
        return (None, None);
    }
    let Ok(url) = reqwest::Url::parse(&format!("{scheme}://{authority}")) else {
        return (Some(authority.to_string()), None);
    };
    let host = url.host_str().map(ToString::to_string);
    let port = url.port_or_known_default();
    (host, port)
}

fn request_client_address(request: &Request<Body>) -> Option<String> {
    forwarded_header_value(request.headers(), "x-forwarded-for")
}

fn build_sync_routes(
    state: Option<anlg_api_sync::AppState>,
    replica_state: anlg_api_sync::ReplicaState,
    cloudsync_rate_limit_state: rate_limit::RateLimitState,
    session_share_rate_limit_state: rate_limit::RateLimitState,
    witness_rate_limit_state: rate_limit::RateLimitState,
    auth_state: AuthState,
) -> Router {
    let replica_routes = anlg_api_sync::replica_router(replica_state.clone())
        .route_layer(middleware::from_fn_with_state(
            cloudsync_rate_limit_state.clone(),
            rate_limit::rate_limit,
        ))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state.clone().with_required_entitlement("hyprnote_pro"),
            auth::require_auth,
        ));
    let witness_routes = anlg_api_sync::e2ee_witness_router(replica_state)
        .route_layer(middleware::from_fn_with_state(
            witness_rate_limit_state,
            rate_limit::wait_for_rate_limit,
        ))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state.clone().with_required_entitlement("hyprnote_pro"),
            auth::require_auth,
        ));
    let replica_routes = replica_routes.merge(witness_routes);

    let Some(state) = state else {
        return replica_routes;
    };

    let cloudsync_routes = anlg_api_sync::cloudsync_router(state.clone())
        .route_layer(middleware::from_fn_with_state(
            cloudsync_rate_limit_state,
            rate_limit::rate_limit,
        ))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state.clone().with_required_entitlement("hyprnote_pro"),
            auth::require_auth,
        ));
    let session_share_routes = anlg_api_sync::session_share_router(state.clone())
        .route_layer(middleware::from_fn_with_state(
            session_share_rate_limit_state.clone(),
            rate_limit::rate_limit,
        ))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state.clone().with_required_entitlement("hyprnote_pro"),
            auth::require_auth,
        ));
    let web_edit_routes = anlg_api_sync::web_edit_router(state)
        .route_layer(middleware::from_fn_with_state(
            session_share_rate_limit_state,
            rate_limit::rate_limit,
        ))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state,
            auth::require_auth,
        ));

    replica_routes
        .merge(cloudsync_routes)
        .merge(session_share_routes)
        .merge(web_edit_routes)
}

async fn app() -> Router {
    let env = env();
    app_with_env(env).await
}

async fn app_with_env(env: &'static crate::env::RuntimeConfig) -> Router {
    let analytics = build_analytics_client(env);

    let llm_config =
        anlg_llm_proxy::LlmProxyConfig::new(&env.llm).with_analytics(analytics.clone());
    let stt_config = anlg_transcribe_proxy::SttProxyConfig::new(&env.stt, &env.supabase)
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
    let build_sync_rate_limit = || {
        let quota = || {
            governor::Quota::with_period(Duration::from_secs(30))
                .unwrap()
                .allow_burst(NonZeroU32::new(20).unwrap())
        };
        rate_limit::RateLimitState::builder()
            .pro(quota())
            .free(quota())
            .build()
    };
    let cloudsync_rate_limit = build_sync_rate_limit();
    let session_share_rate_limit = build_sync_rate_limit();
    let e2ee_witness_rate_limit = rate_limit::RateLimitState::builder()
        .pro(
            governor::Quota::with_period(Duration::from_millis(100))
                .unwrap()
                .allow_burst(NonZeroU32::new(20).unwrap()),
        )
        .free(
            governor::Quota::with_period(Duration::from_millis(100))
                .unwrap()
                .allow_burst(NonZeroU32::new(20).unwrap()),
        )
        .build();
    let shared_notes_rate_limit = rate_limit::IpRateLimitState::new(
        governor::Quota::with_period(Duration::from_secs(1))
            .unwrap()
            .allow_burst(NonZeroU32::new(30).unwrap()),
    );
    let cloud_api_rate_limit = rate_limit::RateLimitState::builder()
        .pro(
            governor::Quota::with_period(Duration::from_millis(200))
                .unwrap()
                .allow_burst(NonZeroU32::new(10).unwrap()),
        )
        .free(
            governor::Quota::with_period(Duration::from_millis(200))
                .unwrap()
                .allow_burst(NonZeroU32::new(10).unwrap()),
        )
        .build();

    let auth_state = AuthState::new(&env.supabase.supabase_url);
    let auth_state_paid = auth_state.clone().with_required_entitlements(
        PAID_ENTITLEMENTS
            .iter()
            .map(|entitlement| (*entitlement).to_string())
            .collect(),
    );
    let auth_state_basic = auth_state.clone();

    let nango_config = env.nango.as_ref().map(|nango| {
        anlg_api_nango::NangoConfig::new(
            nango,
            &env.supabase,
            Some(env.supabase.supabase_service_role_key.clone()),
        )
    });
    let subscription_config = env.subscription.as_ref().map(|(stripe, loops)| {
        anlg_api_subscription::SubscriptionConfig::new(&env.supabase, stripe, loops)
            .with_analytics(analytics.clone())
            .with_durable_cleanup_enabled(env.anarlog_attachment_backup_gc_enabled)
    });
    let research_config = env.research.clone();
    let pyannote_config = env
        .pyannote
        .as_ref()
        .map(anlg_api_pyannote::PyannoteConfig::new);
    let sync_config = anlg_api_sync::SyncConfig::from_env(
        &env.sync,
        &env.supabase.supabase_url,
        &env.supabase.supabase_anon_key,
        &env.supabase.supabase_service_role_key,
    )
    .unwrap_or_else(|error| panic!("Failed to load environment: {error}"));
    let shared_notes_config = anlg_api_sync::SharedNotesConfig::new(
        &env.supabase.supabase_url,
        &env.supabase.supabase_service_role_key,
    )
    .unwrap_or_else(|error| panic!("Failed to load environment: {error}"));
    let (Some(resend_api_key), Some(resend_from_email)) = (
        env.resend.resend_api_key.as_deref(),
        env.resend.resend_from_email.as_deref(),
    ) else {
        panic!(
            "Failed to load environment: RESEND_API_KEY and RESEND_FROM_EMAIL are required for shared note email"
        );
    };
    let shared_notes_config = shared_notes_config
        .with_resend_email(resend_api_key, resend_from_email)
        .unwrap_or_else(|error| panic!("Failed to load environment: {error}"));
    let cloud_api_state = anlg_api_cloud::AppState::new(
        anlg_api_cloud::CloudApiConfig::new(
            &env.supabase.supabase_url,
            &env.supabase.supabase_service_role_key,
        )
        .unwrap_or_else(|error| panic!("Failed to load environment: {error}")),
    );

    use anlg_api_nango::NangoIntegrationId;

    let nango_webhook_routes = match nango_config.clone() {
        Some(config) => {
            let mut forward_handlers = anlg_api_nango::ForwardHandlerRegistry::new();
            forward_handlers.insert(
                anlg_api_nango::Linear::ID.to_string(),
                anlg_api_nango::forward_handler(anlg_linear::webhook::handle),
            );
            Router::new().nest(
                "/nango",
                anlg_api_nango::webhook_router(config, forward_handlers),
            )
        }
        None => Router::new(),
    };

    let webhook_routes = Router::new().merge(nango_webhook_routes).nest(
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

    let replica_state = anlg_api_sync::ReplicaState::new(
        anlg_api_sync::ReplicaConfig::new(
            &env.supabase.supabase_url,
            &env.supabase.supabase_anon_key,
            &env.supabase.supabase_service_role_key,
        )
        .unwrap_or_else(|error| panic!("Failed to load environment: {error}")),
    );
    let sync_state = sync_config.map(anlg_api_sync::AppState::new);
    let sync_routes = build_sync_routes(
        sync_state,
        replica_state,
        cloudsync_rate_limit,
        session_share_rate_limit,
        e2ee_witness_rate_limit,
        auth_state.clone(),
    );
    let shared_notes_state = anlg_api_sync::SharedNotesState::new(shared_notes_config);
    let shared_notes_routes = anlg_api_sync::shared_notes_router(shared_notes_state.clone())
        .route_layer(middleware::from_fn_with_state(
            shared_notes_rate_limit.clone(),
            rate_limit::rate_limit_by_ip,
        ));
    let authenticated_shared_notes_routes =
        anlg_api_sync::authenticated_shared_notes_router(shared_notes_state)
            .route_layer(middleware::from_fn_with_state(
                shared_notes_rate_limit,
                rate_limit::rate_limit_by_ip,
            ))
            .route_layer(middleware::from_fn(auth::sentry_and_analytics))
            .route_layer(middleware::from_fn_with_state(
                auth_state.clone(),
                auth::require_auth,
            ));
    let cloud_api_management_routes = anlg_api_cloud::management_router(cloud_api_state.clone())
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state.clone(),
            auth::require_auth,
        ));
    let cloud_api_connector_routes = anlg_api_cloud::connector_router(cloud_api_state.clone())
        .route_layer(middleware::from_fn_with_state(
            cloud_api_rate_limit,
            rate_limit::rate_limit,
        ))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            cloud_api_state,
            anlg_api_cloud::require_cloud_api_key,
        ));

    let integration_routes = match nango_config.clone() {
        Some(config) => {
            let nango_connection_state = anlg_api_nango::NangoConnectionState::from_config(&config);
            Router::new()
                .nest("/calendar", anlg_api_calendar::router())
                .nest("/mail", anlg_api_mail::router())
                .nest("/messenger", anlg_api_messenger::router())
                .nest("/notion", anlg_api_notion::router())
                .nest("/ticket", anlg_api_ticket::router())
                .nest("/zoom", anlg_api_zoom::router())
                .merge(anlg_api_meeting_import::router())
                .nest("/nango", anlg_api_nango::session_router(config))
                .layer(axum::Extension(nango_connection_state))
                .route_layer(middleware::from_fn(auth::sentry_and_analytics))
                .route_layer(middleware::from_fn_with_state(
                    auth_state_paid,
                    auth::require_auth,
                ))
        }
        None => Router::new(),
    };

    let integration_management_routes = match nango_config {
        Some(config) => Router::new()
            .nest("/nango", anlg_api_nango::management_router(config))
            .route_layer(middleware::from_fn(auth::sentry_and_analytics))
            .route_layer(middleware::from_fn_with_state(
                auth_state_basic.clone(),
                auth::require_auth,
            )),
        None => Router::new(),
    };

    let stt_routes = Router::new()
        .merge(anlg_transcribe_proxy::listen_router(stt_config.clone()))
        .nest("/stt", anlg_transcribe_proxy::router(stt_config))
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

    let subscription_routes = match subscription_config {
        Some(config) => {
            let router = anlg_api_subscription::router(config);
            Router::new()
                .nest("/subscription", router.clone())
                .nest("/rpc", router.clone())
                .nest("/billing", router)
        }
        None => Router::new(),
    };
    let auth_routes = Router::new()
        .merge(stt_routes)
        .merge(llm_routes)
        .merge(subscription_routes)
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state_basic,
            auth::require_auth,
        ));

    Router::new()
        .route("/health", axum::routing::get(version))
        .route("/openapi.json", axum::routing::get(openapi_json))
        .merge(webhook_routes)
        .merge(paid_routes)
        .merge(shared_notes_routes)
        .merge(authenticated_shared_notes_routes)
        .merge(cloud_api_management_routes)
        .merge(cloud_api_connector_routes)
        .nest("/sync", sync_routes)
        .merge(integration_routes)
        .merge(integration_management_routes)
        .merge(auth_routes)
        .layer(
            CorsLayer::new()
                .allow_origin(cors::Any)
                .allow_methods(cors::Any)
                .allow_headers(cors::Any)
                .expose_headers([axum::http::header::HeaderName::from_static(
                    REQUEST_ID_HEADER,
                )]),
        )
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(PropagateRequestIdLayer::x_request_id())
                .layer(NewSentryLayer::<Request<Body>>::new_from_top())
                .layer(SentryHttpLayer::new().enable_transaction())
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(|request: &Request<Body>| {
                            let path = request.uri().path();

                            if path == "/health" {
                                return tracing::Span::none();
                            }

                            let method = request.method();
                            let matched_path = request
                                .extensions()
                                .get::<MatchedPath>()
                                .map(MatchedPath::as_str)
                                .unwrap_or(path);
                            let scheme = request_scheme(request);
                            let (server_address, server_port) =
                                request_server_endpoint(request, &scheme);
                            let client_address = request_client_address(request);
                            let span_op = match path {
                                p if p.starts_with("/llm")
                                    || p.starts_with("/chat/completions") =>
                                {
                                    "http.server.llm"
                                }
                                p if p.starts_with("/stt") || p.starts_with("/listen") => {
                                    "http.server.stt"
                                }
                                _ => "http.server",
                            };

                            let span = tracing::info_span!(
                                "http_request",
                                http.request.method = %method,
                                http.route = %matched_path,
                                url.path = %path,
                                url.scheme = %scheme,
                                http.response.status_code = tracing::field::Empty,
                                server.address = tracing::field::Empty,
                                server.port = tracing::field::Empty,
                                client.address = tracing::field::Empty,
                                anarlog.subsystem = "edge",
                                enduser.id = tracing::field::Empty,
                                enduser.pseudo.id = tracing::field::Empty,
                                anarlog.stt.provider.name = tracing::field::Empty,
                                anarlog.stt.routing_strategy = tracing::field::Empty,
                                anarlog.stt.model = tracing::field::Empty,
                                anarlog.stt.language_codes = tracing::field::Empty,
                                anarlog.audio.sample_rate_hz = tracing::field::Empty,
                                anarlog.audio.channel_count = tracing::field::Empty,
                                gen_ai.provider.name = tracing::field::Empty,
                                anarlog.gen_ai.request.streaming = tracing::field::Empty,
                                anarlog.gen_ai.request.message_count = tracing::field::Empty,
                                anarlog.request.id = tracing::field::Empty,
                                error.type = tracing::field::Empty,
                                otel.status_code = tracing::field::Empty,
                                otel.kind = "server",
                                otel.name = %format!("{} {}", method, matched_path),
                                span.op = %span_op,
                            );
                            if let Some(server_address) = server_address.as_deref() {
                                span.record("server.address", server_address);
                            }
                            if let Some(server_port) = server_port {
                                span.record("server.port", server_port as i64);
                            }
                            if let Some(client_address) = client_address.as_deref() {
                                span.record("client.address", client_address);
                            }
                            anlg_observability::set_remote_parent(&span, request.headers());
                            span
                        })
                        .on_request(|request: &Request<Body>, span: &tracing::Span| {
                            // Skip logging for health checks
                            if request.uri().path() == "/health" {
                                return;
                            }
                            if let Some(request_id) = request
                                .headers()
                                .get(REQUEST_ID_HEADER)
                                .and_then(|v| v.to_str().ok())
                            {
                                span.record("anarlog.request.id", request_id);
                            }
                            configure_sentry_trace_scope(span, env, SystemTime::now());
                            tracing::info!(
                                parent: span,
                                http.request.method = %request.method(),
                                url.path = %request.uri().path(),
                                "http_request_started"
                            );
                        })
                        .on_response(
                            |response: &axum::http::Response<axum::body::Body>,
                             latency: std::time::Duration,
                             span: &tracing::Span| {
                                if span.is_disabled() {
                                    return;
                                }
                                span.record(
                                    "http.response.status_code",
                                    response.status().as_u16() as i64,
                                );
                                if response.status().is_server_error() {
                                    anlg_observability::mark_span_as_error(
                                        span,
                                        &response.status().as_u16().to_string(),
                                    );
                                }
                                tracing::info!(
                                    parent: span,
                                    http.response.status_code = %response.status().as_u16(),
                                    anarlog.duration_ms = %latency.as_millis(),
                                    "http_request_finished"
                                );
                            },
                        )
                        .on_failure(
                            |failure_class: ServerErrorsFailureClass,
                             latency: std::time::Duration,
                             span: &tracing::Span| {
                                if span.is_disabled() {
                                    return;
                                }
                                let error_type = match &failure_class {
                                    ServerErrorsFailureClass::StatusCode(status) => {
                                        status.as_u16().to_string()
                                    }
                                    ServerErrorsFailureClass::Error(_) => {
                                        "http_server_failure".to_string()
                                    }
                                };
                                anlg_observability::mark_span_as_error(span, error_type.as_str());
                                tracing::error!(
                                    parent: span,
                                    error.type = %error_type,
                                    error = %failure_class,
                                    anarlog.duration_ms = %latency.as_millis(),
                                    "http_request_failed"
                                );
                            },
                        ),
                ),
        )
}

fn build_analytics_client(env: &Env) -> Arc<anlg_analytics::AnalyticsClient> {
    let mut builder = anlg_analytics::AnalyticsClientBuilder::default();
    if cfg!(debug_assertions) {
        tracing::info!("analytics: dev mode, printing events as tracing");
    } else {
        let key = env
            .posthog_api_key
            .as_ref()
            .expect("POSTHOG_API_KEY is required in production");
        builder = builder.with_posthog(key);
    }
    Arc::new(builder.build())
}

fn main() -> std::io::Result<()> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    let _ = openapi::write_openapi_json();

    let env = env();

    let _guard = sentry::init(sentry::ClientOptions {
        dsn: env.sentry_dsn.as_ref().and_then(|s| s.parse().ok()),
        release: option_env!("APP_VERSION").map(|v| format!("anarlog-api@{}", v).into()),
        environment: Some(
            if cfg!(debug_assertions) {
                "development"
            } else {
                "production"
            }
            .into(),
        ),
        traces_sample_rate: 1.0,
        sample_rate: 1.0,
        send_default_pii: false,
        auto_session_tracking: true,
        session_mode: sentry::SessionMode::Request,
        attach_stacktrace: true,
        max_breadcrumbs: 100,
        before_send: Some(Arc::new(anlg_user_error::drop_user_error_event)),
        ..Default::default()
    });

    sentry::configure_scope(|scope| {
        scope.set_tag("service.namespace", "anarlog");
        scope.set_tag("service.name", "api");
    });

    let observability = observability::init("api", &env.observability);

    anlg_transcribe_proxy::ApiKeys::from(&env.stt.stt).log_configured_providers();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let addr = SocketAddr::from(([0, 0, 0, 0], env.port));
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            let app = app().await;
            let cancellation = CancellationToken::new();
            let worker_task = env.anarlog_attachment_backup_gc_enabled.then(|| {
                let (stripe, loops) = env
                    .subscription
                    .as_ref()
                    .expect("cleanup requires Stripe and Loops configuration");
                let cloudsync_cleanup = anlg_api_subscription::CloudsyncCleanupConfig::new(
                    env.sync
                        .sqlitecloud_project_url
                        .as_deref()
                        .unwrap_or_default(),
                    env.sync
                        .sqlitecloud_token_issuer_api_key
                        .as_deref()
                        .unwrap_or_default(),
                    env.sync
                        .anarlog_cloudsync_e2ee_database_id
                        .as_deref()
                        .unwrap_or_default(),
                    env.sqlitecloud_cloudsync_management_api_key
                        .as_deref()
                        .unwrap_or_default(),
                )
                .unwrap_or_else(|error| panic!("Failed to load environment: {error}"));
                let config =
                    anlg_api_subscription::SubscriptionConfig::new(&env.supabase, stripe, loops)
                        .with_cloudsync_cleanup(cloudsync_cleanup);
                let worker = anlg_api_subscription::CleanupWorker::new(&config);
                let worker_cancellation = cancellation.clone();
                tokio::spawn(worker.run(worker_cancellation))
            });
            tracing::info!(addr = %addr, "server_listening");

            let shutdown_cancellation = cancellation.clone();
            let server_result = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown_signal().await;
                    shutdown_cancellation.cancel();
                })
                .await;
            cancellation.cancel();
            if let Some(mut worker_task) = worker_task {
                if tokio::time::timeout(Duration::from_secs(20), &mut worker_task)
                    .await
                    .is_err()
                {
                    tracing::warn!("durable_cleanup_worker_shutdown_timed_out");
                    worker_task.abort();
                }
            }
            server_result.unwrap();
        });

    if let Some(client) = sentry::Hub::current().client() {
        client.close(Some(Duration::from_secs(2)));
    }
    observability.shutdown();

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install CTRL+C signal handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM signal handler")
            .recv()
            .await;
    };

    #[cfg(unix)]
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    #[cfg(not(unix))]
    ctrl_c.await;

    tracing::info!("shutdown_signal_received");
}

async fn openapi_json() -> axum::Json<utoipa::openapi::OpenApi> {
    axum::Json(openapi::openapi())
}

async fn version() -> &'static str {
    option_env!("APP_VERSION").unwrap_or("unknown")
}

fn configure_sentry_trace_scope(span: &tracing::Span, env: &Env, request_started_at: SystemTime) {
    let Some(trace_identifiers) = anlg_observability::span_identifiers(span) else {
        return;
    };

    let trace_url = build_honeycomb_trace_url(env, &trace_identifiers, request_started_at);
    sentry::configure_scope(|scope| {
        scope.set_tag(
            "anarlog.honeycomb.trace_id",
            trace_identifiers.trace_id.as_str(),
        );
        scope.set_tag(
            "anarlog.honeycomb.span_id",
            trace_identifiers.span_id.as_str(),
        );
        if let Some(trace_url) = trace_url.as_deref() {
            scope.set_tag("anarlog.honeycomb.trace_url", trace_url);
        }

        let mut context = std::collections::BTreeMap::new();
        context.insert("trace_id".into(), Value::String(trace_identifiers.trace_id));
        context.insert("span_id".into(), Value::String(trace_identifiers.span_id));
        if let Some(trace_url) = trace_url {
            context.insert("trace_url".into(), Value::String(trace_url));
        }
        scope.set_context("anarlog.honeycomb", Context::Other(context));
    });
}

fn build_honeycomb_trace_url(
    env: &Env,
    trace_identifiers: &anlg_observability::TraceIdentifiers,
    request_started_at: SystemTime,
) -> Option<String> {
    let team = env.observability.honeycomb_ui_team.as_deref()?;
    let environment = env.observability.honeycomb_ui_environment.as_deref()?;
    let base_url = env
        .observability
        .honeycomb_ui_base_url
        .as_deref()
        .unwrap_or("https://ui.honeycomb.io")
        .trim_end_matches('/');
    let trace_start_ts = request_started_at
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs()
        .to_string();

    let mut url = url::Url::parse(&format!(
        "{base_url}/{team}/environments/{environment}/trace"
    ))
    .ok()?;
    url.query_pairs_mut()
        .append_pair("trace_id", trace_identifiers.trace_id.as_str())
        .append_pair("span", trace_identifiers.span_id.as_str())
        .append_pair("trace_start_ts", trace_start_ts.as_str());

    Some(url.into())
}

#[cfg(test)]
mod tests;

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

async fn app() -> Router {
    let env = env();

    let analytics = build_analytics_client(env);

    let llm_config =
        hypr_llm_proxy::LlmProxyConfig::new(&env.llm).with_analytics(analytics.clone());
    let stt_config = hypr_transcribe_proxy::SttProxyConfig::new(&env.stt, &env.supabase)
        .with_hyprnote_routing(hypr_transcribe_proxy::HyprnoteRoutingConfig::default())
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

    let auth_state_pro =
        AuthState::new(&env.supabase.supabase_url).with_required_entitlement("hyprnote_pro");
    let auth_state_basic = AuthState::new(&env.supabase.supabase_url);
    let auth_state_support = AuthState::new(&env.supabase.supabase_url);

    let nango_config = hypr_api_nango::NangoConfig::new(
        &env.nango,
        &env.supabase,
        Some(env.supabase.supabase_service_role_key.clone()),
    );
    let nango_connection_state = hypr_api_nango::NangoConnectionState::from_config(&nango_config);
    let subscription_config =
        hypr_api_subscription::SubscriptionConfig::new(&env.supabase, &env.stripe, &env.loops)
            .with_analytics(analytics.clone());
    let support_config = hypr_api_support::SupportConfig::new(
        &env.github_app,
        &env.llm,
        &env.support_database,
        &env.stripe,
        &env.supabase,
        &env.chatwoot,
        auth_state_support.clone(),
    );
    let research_config = hypr_api_research::ResearchConfig {
        exa_api_key: env.exa_api_key.clone(),
        jina_api_key: env.jina_api_key.clone(),
    };

    let webhook_routes = Router::new()
        .nest(
            "/nango",
            hypr_api_nango::webhook_router(nango_config.clone()),
        )
        .nest(
            "/stt",
            hypr_transcribe_proxy::callback_router(stt_config.clone()),
        );

    let auth_state_integration =
        AuthState::new(&env.supabase.supabase_url).with_required_entitlement("hyprnote_pro");

    let pro_routes = Router::new()
        .merge(hypr_api_research::router(research_config))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state_pro,
            auth::require_auth,
        ));

    let integration_routes = Router::new()
        .nest("/calendar", hypr_api_calendar::router())
        .nest("/mail", hypr_api_mail::router())
        .nest("/ticket", hypr_api_ticket::router())
        .nest("/nango", hypr_api_nango::router(nango_config.clone()))
        .layer(axum::Extension(nango_connection_state))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state_integration,
            auth::require_auth,
        ));

    let stt_routes = Router::new()
        .merge(hypr_transcribe_proxy::listen_router(stt_config.clone()))
        .nest("/stt", hypr_transcribe_proxy::router(stt_config))
        .route_layer(middleware::from_fn_with_state(
            stt_rate_limit,
            rate_limit::rate_limit,
        ));

    let llm_routes = Router::new()
        .merge(hypr_llm_proxy::chat_completions_router(llm_config.clone()))
        .nest("/llm", hypr_llm_proxy::router(llm_config))
        .route_layer(middleware::from_fn_with_state(
            llm_rate_limit,
            rate_limit::rate_limit,
        ));

    let subscription_router = hypr_api_subscription::router(subscription_config);
    let auth_routes = Router::new()
        .merge(stt_routes)
        .merge(llm_routes)
        .nest("/subscription", subscription_router.clone())
        .nest("/rpc", subscription_router.clone())
        .nest("/billing", subscription_router)
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state_basic,
            auth::require_auth,
        ));

    let support_routes = Router::new()
        .merge(hypr_api_support::router(support_config).await)
        .layer(middleware::from_fn_with_state(
            auth_state_support.clone(),
            auth::optional_auth,
        ));

    Router::new()
        .route("/health", axum::routing::get(version))
        .route("/openapi.json", axum::routing::get(openapi_json))
        .merge(support_routes)
        .merge(webhook_routes)
        .merge(pro_routes)
        .merge(integration_routes)
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
                                hyprnote.subsystem = "edge",
                                enduser.id = tracing::field::Empty,
                                enduser.pseudo.id = tracing::field::Empty,
                                hyprnote.stt.provider.name = tracing::field::Empty,
                                hyprnote.stt.routing_strategy = tracing::field::Empty,
                                hyprnote.stt.model = tracing::field::Empty,
                                hyprnote.stt.language_codes = tracing::field::Empty,
                                hyprnote.audio.sample_rate_hz = tracing::field::Empty,
                                hyprnote.audio.channel_count = tracing::field::Empty,
                                gen_ai.provider.name = tracing::field::Empty,
                                hyprnote.gen_ai.request.streaming = tracing::field::Empty,
                                hyprnote.gen_ai.request.message_count = tracing::field::Empty,
                                hyprnote.request.id = tracing::field::Empty,
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
                            hypr_observability::set_remote_parent(&span, request.headers());
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
                                span.record("hyprnote.request.id", request_id);
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
                                    hypr_observability::mark_span_as_error(
                                        span,
                                        &response.status().as_u16().to_string(),
                                    );
                                }
                                tracing::info!(
                                    parent: span,
                                    http.response.status_code = %response.status().as_u16(),
                                    hyprnote.duration_ms = %latency.as_millis(),
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
                                hypr_observability::mark_span_as_error(span, error_type.as_str());
                                tracing::error!(
                                    parent: span,
                                    error.type = %error_type,
                                    error = %failure_class,
                                    hyprnote.duration_ms = %latency.as_millis(),
                                    "http_request_failed"
                                );
                            },
                        ),
                ),
        )
}

fn build_analytics_client(env: &Env) -> Arc<hypr_analytics::AnalyticsClient> {
    let mut builder = hypr_analytics::AnalyticsClientBuilder::default();
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
        release: option_env!("APP_VERSION").map(|v| format!("hyprnote-api@{}", v).into()),
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
        send_default_pii: true,
        auto_session_tracking: true,
        session_mode: sentry::SessionMode::Request,
        attach_stacktrace: true,
        max_breadcrumbs: 100,
        ..Default::default()
    });

    sentry::configure_scope(|scope| {
        scope.set_tag("service.namespace", "hyprnote");
        scope.set_tag("service.name", "api");
    });

    let observability = observability::init("api", &env.observability);

    hypr_transcribe_proxy::ApiKeys::from(&env.stt.stt).log_configured_providers();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let addr = SocketAddr::from(([0, 0, 0, 0], env.port));
            tracing::info!(addr = %addr, "server_listening");

            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            axum::serve(listener, app().await)
                .with_graceful_shutdown(shutdown_signal())
                .await
                .unwrap();
        });

    if let Some(client) = sentry::Hub::current().client() {
        client.close(Some(Duration::from_secs(2)));
    }
    observability.shutdown();

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
    tracing::info!("shutdown_signal_received");
}

async fn openapi_json() -> axum::Json<utoipa::openapi::OpenApi> {
    axum::Json(openapi::openapi())
}

async fn version() -> &'static str {
    option_env!("APP_VERSION").unwrap_or("unknown")
}

fn configure_sentry_trace_scope(span: &tracing::Span, env: &Env, request_started_at: SystemTime) {
    let Some(trace_identifiers) = hypr_observability::span_identifiers(span) else {
        return;
    };

    let trace_url = build_honeycomb_trace_url(env, &trace_identifiers, request_started_at);
    sentry::configure_scope(|scope| {
        scope.set_tag(
            "hyprnote.honeycomb.trace_id",
            trace_identifiers.trace_id.as_str(),
        );
        scope.set_tag(
            "hyprnote.honeycomb.span_id",
            trace_identifiers.span_id.as_str(),
        );
        if let Some(trace_url) = trace_url.as_deref() {
            scope.set_tag("hyprnote.honeycomb.trace_url", trace_url);
        }

        let mut context = std::collections::BTreeMap::new();
        context.insert("trace_id".into(), Value::String(trace_identifiers.trace_id));
        context.insert("span_id".into(), Value::String(trace_identifiers.span_id));
        if let Some(trace_url) = trace_url {
            context.insert("trace_url".into(), Value::String(trace_url));
        }
        scope.set_context("hyprnote.honeycomb", Context::Other(context));
    });
}

fn build_honeycomb_trace_url(
    env: &Env,
    trace_identifiers: &hypr_observability::TraceIdentifiers,
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

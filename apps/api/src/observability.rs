use std::collections::HashMap;

use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use serde::Deserialize;
use tracing_subscriber::prelude::*;

#[derive(Deserialize)]
pub struct Env {
    #[serde(default, deserialize_with = "hypr_api_env::filter_empty")]
    pub otel_service_name: Option<String>,
    #[serde(flatten)]
    direct: DirectHoneycombEnv,
    #[serde(flatten)]
    collector: OtelCollectorEnv,
    #[serde(default, deserialize_with = "hypr_api_env::filter_empty")]
    pub honeycomb_ui_base_url: Option<String>,
    #[serde(default, deserialize_with = "hypr_api_env::filter_empty")]
    pub honeycomb_ui_team: Option<String>,
    #[serde(default, deserialize_with = "hypr_api_env::filter_empty")]
    pub honeycomb_ui_environment: Option<String>,
}

#[derive(Deserialize)]
struct DirectHoneycombEnv {
    #[serde(default, deserialize_with = "hypr_api_env::filter_empty")]
    honeycomb_api_key: Option<String>,
    #[serde(default, deserialize_with = "hypr_api_env::filter_empty")]
    honeycomb_api_endpoint: Option<String>,
    #[serde(default, deserialize_with = "hypr_api_env::filter_empty")]
    honeycomb_dataset: Option<String>,
}

#[derive(Deserialize)]
struct OtelCollectorEnv {
    #[serde(default, deserialize_with = "hypr_api_env::filter_empty")]
    otel_exporter_otlp_endpoint: Option<String>,
}

pub struct ObservabilityGuard {
    otel_provider: Option<SdkTracerProvider>,
}

pub fn init(service_name: &str, env: &Env) -> ObservabilityGuard {
    hypr_observability::install_trace_context_propagator();
    let otel_provider = init_otel_tracer_provider(service_name, env);
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,tower_http=debug".into());

    if let Some(provider) = otel_provider.as_ref() {
        let tracer = provider.tracer(service_name.to_string());
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .with(sentry::integrations::tracing::layer())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .with(sentry::integrations::tracing::layer())
            .init();
    }

    ObservabilityGuard { otel_provider }
}

impl ObservabilityGuard {
    pub fn shutdown(self) {
        if let Some(provider) = self.otel_provider
            && let Err(e) = provider.shutdown()
        {
            tracing::warn!(error.message = %e, "otel_tracer_shutdown_failed");
        }
    }
}

fn init_otel_tracer_provider(service_name: &str, env: &Env) -> Option<SdkTracerProvider> {
    let export_config = trace_export_config(env)?;

    let exporter_builder = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(trace_export_endpoint(&export_config.endpoint))
        .with_headers(export_config.headers);
    let exporter = exporter_builder.build().ok()?;

    let configured_service_name = env
        .otel_service_name
        .clone()
        .unwrap_or_else(|| service_name.to_string());
    let environment = if cfg!(debug_assertions) {
        "development"
    } else {
        "production"
    };
    let version = option_env!("APP_VERSION").unwrap_or("unknown");

    let resource = Resource::builder_empty()
        .with_attributes([
            KeyValue::new("service.namespace", "hyprnote"),
            KeyValue::new("service.name", configured_service_name),
            KeyValue::new("service.version", version.to_string()),
            KeyValue::new("deployment.environment", environment),
        ])
        .build();

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    global::set_tracer_provider(provider.clone());
    Some(provider)
}

struct TraceExportConfig {
    endpoint: String,
    headers: HashMap<String, String>,
}

fn trace_export_config(env: &Env) -> Option<TraceExportConfig> {
    if let Some(config) = env.direct.trace_export_config() {
        return Some(config);
    }

    env.collector.trace_export_config()
}

impl DirectHoneycombEnv {
    fn trace_export_config(&self) -> Option<TraceExportConfig> {
        let api_key = self.honeycomb_api_key.clone()?;
        let mut headers = HashMap::from([("x-honeycomb-team".to_string(), api_key)]);
        if let Some(dataset) = self.honeycomb_dataset.clone() {
            headers.insert("x-honeycomb-dataset".to_string(), dataset);
        }

        Some(TraceExportConfig {
            endpoint: normalize_endpoint(
                self.honeycomb_api_endpoint
                    .as_deref()
                    .unwrap_or("https://api.honeycomb.io"),
                "https",
            ),
            headers,
        })
    }
}

impl OtelCollectorEnv {
    fn trace_export_config(&self) -> Option<TraceExportConfig> {
        Some(TraceExportConfig {
            endpoint: normalize_endpoint(self.otel_exporter_otlp_endpoint.as_deref()?, "http"),
            headers: HashMap::new(),
        })
    }
}

fn normalize_endpoint(endpoint: &str, default_scheme: &str) -> String {
    if endpoint.contains("://") {
        endpoint.to_string()
    } else {
        format!("{default_scheme}://{endpoint}")
    }
}

fn trace_export_endpoint(base_url: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.ends_with("/v1/traces") {
        return base_url.to_string();
    }

    format!("{base_url}/v1/traces")
}

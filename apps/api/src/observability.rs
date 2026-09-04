use std::borrow::Cow;
use std::collections::HashMap;
use std::time::Duration;

use opentelemetry::global;
use opentelemetry::trace::{Status, TracerProvider as _};
use opentelemetry::{Array, KeyValue, Value};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::{SdkTracerProvider, SpanData, SpanExporter};
use serde::Deserialize;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::fmt::time::{FormatTime, SystemTime};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;

#[derive(Deserialize)]
pub struct Env {
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    pub otel_service_name: Option<String>,
    #[serde(flatten)]
    direct: DirectHoneycombEnv,
    #[serde(flatten)]
    collector: OtelCollectorEnv,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    pub honeycomb_ui_base_url: Option<String>,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    pub honeycomb_ui_team: Option<String>,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    pub honeycomb_ui_environment: Option<String>,
}

#[derive(Deserialize)]
struct DirectHoneycombEnv {
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    honeycomb_api_key: Option<String>,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    honeycomb_api_endpoint: Option<String>,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    honeycomb_dataset: Option<String>,
}

#[derive(Deserialize)]
struct OtelCollectorEnv {
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    otel_exporter_otlp_endpoint: Option<String>,
}

pub struct ObservabilityGuard {
    otel_provider: Option<SdkTracerProvider>,
}

pub fn init(service_name: &str, env: &Env) -> ObservabilityGuard {
    anlg_observability::install_trace_context_propagator();
    let otel_provider = init_otel_tracer_provider(service_name, env);
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,tower_http=debug".into());

    if let Some(provider) = otel_provider.as_ref() {
        let tracer = provider.tracer(service_name.to_string());
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().event_format(SafeEventFormatter))
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .with(sentry::integrations::tracing::layer())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().event_format(SafeEventFormatter))
            .with(sentry::integrations::tracing::layer())
            .init();
    }

    ObservabilityGuard { otel_provider }
}

struct SafeEventFormatter;

impl<S, N> FormatEvent<S, N> for SafeEventFormatter
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let mut fields = SafeEventVisitor::default();
        event.record(&mut fields);
        if fields.user_error_detected {
            return Ok(());
        }

        let metadata = event.metadata();
        SystemTime.format_time(&mut writer)?;
        write!(writer, " {} ", metadata.level(),)?;
        if let Some(scope) = ctx.event_scope() {
            for span in scope.from_root() {
                write!(
                    writer,
                    "{}:",
                    sanitize_telemetry_name(span.metadata().name(), "span")
                )?;
            }
            write!(writer, " ")?;
        }
        write!(
            writer,
            "{} {}",
            metadata.target(),
            fields.message.as_deref().unwrap_or("event")
        )?;
        for (key, value) in fields.safe_fields {
            write!(writer, " {key}={value}")?;
        }
        writeln!(writer)
    }
}

#[derive(Default)]
struct SafeEventVisitor {
    user_error_detected: bool,
    message: Option<String>,
    safe_fields: Vec<(String, String)>,
}

impl SafeEventVisitor {
    fn record_string(&mut self, field: &Field, value: &str) {
        self.user_error_detected |= anlg_user_error::is_user_error_text(value);
        if field.name() == "message" {
            if is_safe_event_message(value) {
                self.message = Some(value.to_string());
            }
            return;
        }

        let value = Value::String(value.to_string().into());
        if !is_sensitive_attribute_key(field.name())
            && is_safe_attribute_value(field.name(), &value)
        {
            self.safe_fields
                .push((field.name().to_string(), value.to_string()));
        }
    }

    fn record_number(&mut self, field: &Field, value: impl ToString) {
        if !is_sensitive_attribute_key(field.name()) {
            self.safe_fields
                .push((field.name().to_string(), value.to_string()));
        }
    }
}

impl Visit for SafeEventVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_string(field, value);
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_number(field, value);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_number(field, value);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_number(field, value);
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_number(field, value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let value = format!("{value:?}");
        let value = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .unwrap_or(&value);
        self.record_string(field, value);
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        if !self.user_error_detected {
            self.user_error_detected = anlg_user_error::is_user_error_text(&value.to_string());
        }
        let _ = field;
    }
}

impl ObservabilityGuard {
    pub fn shutdown(self) {
        if let Some(provider) = self.otel_provider
            && let Err(e) = provider.shutdown()
        {
            tracing::warn!(error = %e, "otel_tracer_shutdown_failed");
        }
    }
}

fn init_otel_tracer_provider(service_name: &str, env: &Env) -> Option<SdkTracerProvider> {
    let export_config = trace_export_config(env)?;

    let exporter_builder = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(trace_export_endpoint(&export_config.endpoint))
        .with_headers(export_config.headers);
    let exporter = SanitizingSpanExporter::new(exporter_builder.build().ok()?);

    let configured_service_name = env
        .otel_service_name
        .clone()
        .unwrap_or_else(|| service_name.to_string());
    let configured_service_name = sanitize_telemetry_name(&configured_service_name, "api");
    let environment = if cfg!(debug_assertions) {
        "development"
    } else {
        "production"
    };
    let version = option_env!("APP_VERSION").unwrap_or("unknown");

    let resource = Resource::builder_empty()
        .with_attributes([
            KeyValue::new("service.namespace", "anarlog"),
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

#[derive(Debug)]
struct SanitizingSpanExporter<E> {
    inner: E,
}

impl<E> SanitizingSpanExporter<E> {
    fn new(inner: E) -> Self {
        Self { inner }
    }
}

impl<E: SpanExporter> SpanExporter for SanitizingSpanExporter<E> {
    fn export(
        &self,
        mut batch: Vec<SpanData>,
    ) -> impl std::future::Future<Output = OTelSdkResult> + Send {
        for span in &mut batch {
            span.name = Cow::Owned(sanitize_span_name(&span.name));
            let span_has_user_error = span
                .attributes
                .iter()
                .any(|attribute| value_contains_user_error(&attribute.value));
            sanitize_span_status(&mut span.status, span_has_user_error);
            span.attributes = sanitize_attributes(std::mem::take(&mut span.attributes));
            span.events.events.retain(|event| {
                !anlg_user_error::is_user_error_text(&event.name)
                    && !event
                        .attributes
                        .iter()
                        .any(|attribute| value_contains_user_error(&attribute.value))
            });
            for event in &mut span.events.events {
                event.name = Cow::Owned(sanitize_telemetry_name(&event.name, "event"));
                event.attributes = sanitize_attributes(std::mem::take(&mut event.attributes));
            }
        }

        self.inner.export(batch)
    }

    fn shutdown_with_timeout(&mut self, timeout: Duration) -> OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }

    fn force_flush(&mut self) -> OTelSdkResult {
        self.inner.force_flush()
    }

    fn set_resource(&mut self, resource: &Resource) {
        self.inner.set_resource(resource);
    }
}

fn sanitize_span_status(status: &mut Status, span_has_user_error: bool) {
    let Status::Error { description } = status else {
        return;
    };

    if span_has_user_error || anlg_user_error::is_user_error_text(description) {
        *status = Status::Unset;
    } else {
        *description = Cow::Borrowed("operation_failed");
    }
}

fn value_contains_user_error(value: &Value) -> bool {
    match value {
        Value::String(value) => anlg_user_error::is_user_error_text(value.as_str()),
        Value::Array(Array::String(values)) => values
            .iter()
            .any(|value| anlg_user_error::is_user_error_text(value.as_str())),
        _ => false,
    }
}

fn sanitize_attributes(attributes: Vec<KeyValue>) -> Vec<KeyValue> {
    attributes
        .into_iter()
        .filter(|attribute| {
            let key = attribute.key.as_str();
            !is_sensitive_attribute_key(key) && is_safe_attribute_value(key, &attribute.value)
        })
        .collect()
}

fn is_sensitive_attribute_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "anarlog.stt.provider.name"
            | "anarlog.error.stage"
            | "anarlog.operation"
            | "anarlog.surface"
            | "db.system.name"
            | "deployment.environment"
            | "error.code"
            | "error.type"
            | "gen_ai.provider.name"
            | "http.request.method"
            | "http.response.status_code"
            | "http.route"
            | "network.protocol.name"
            | "network.protocol.version"
            | "rpc.method"
            | "rpc.service"
            | "rpc.system"
            | "service.name"
            | "service.namespace"
            | "service.peer.name"
            | "service.version"
    ) {
        return false;
    }

    let segments = normalized
        .split(['.', '_', '-'])
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    [
        "account",
        "address",
        "body",
        "contact",
        "content",
        "customer",
        "email",
        "file",
        "identity",
        "message",
        "name",
        "owner",
        "path",
        "prompt",
        "query",
        "request",
        "secret",
        "session",
        "team",
        "text",
        "token",
        "transcript",
        "url",
        "user",
        "workspace",
    ]
    .iter()
    .any(|sensitive| segments.contains(sensitive))
        || segments.contains(&"id")
        || normalized == "error"
        || normalized == "exception.message"
}

fn is_safe_attribute_value(key: &str, value: &Value) -> bool {
    match value {
        Value::Bool(_) | Value::I64(_) | Value::F64(_) => true,
        Value::String(value) => {
            is_safe_string_attribute_key(key) && is_safe_telemetry_token(value.as_str())
        }
        Value::Array(Array::Bool(_) | Array::I64(_) | Array::F64(_)) => true,
        Value::Array(Array::String(values)) => {
            is_safe_string_attribute_key(key)
                && values
                    .iter()
                    .all(|value| is_safe_telemetry_token(value.as_str()))
        }
        _ => false,
    }
}

fn is_safe_string_attribute_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "anarlog.error.stage"
            | "anarlog.operation"
            | "anarlog.stt.provider.name"
            | "anarlog.surface"
            | "db.system.name"
            | "deployment.environment"
            | "error.code"
            | "error.type"
            | "gen_ai.provider.name"
            | "http.request.method"
            | "http.route"
            | "network.protocol.name"
            | "network.protocol.version"
            | "rpc.method"
            | "rpc.service"
            | "rpc.system"
            | "service.name"
            | "service.namespace"
            | "service.peer.name"
            | "service.version"
    )
}

fn is_safe_telemetry_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && !value.contains("..")
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'_' | b'-' | b'.' | b':' | b'/' | b'{' | b'}' | b'<' | b'>'
                )
        })
        && !value.split('/').any(looks_like_opaque_identifier)
}

fn looks_like_opaque_identifier(value: &str) -> bool {
    let uuid_like = value.len() == 36
        && value.bytes().filter(|byte| *byte == b'-').count() == 4
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-');
    let long_token = value.len() >= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));

    uuid_like || long_token || value.parse::<u64>().is_ok_and(|number| number > 9999)
}

fn sanitize_telemetry_name(value: &str, fallback: &str) -> String {
    if is_safe_telemetry_token(value) {
        value.to_string()
    } else {
        fallback.to_string()
    }
}

fn sanitize_span_name(value: &str) -> String {
    if let Some((method, route)) = value.split_once(' ')
        && matches!(
            method,
            "CONNECT" | "DELETE" | "GET" | "HEAD" | "OPTIONS" | "PATCH" | "POST" | "PUT" | "TRACE"
        )
        && is_safe_telemetry_token(route)
    {
        return value.to_string();
    }

    sanitize_telemetry_name(value, "operation")
}

fn is_safe_event_message(value: &str) -> bool {
    is_safe_telemetry_token(value)
        || matches!(
            value,
            "started processing request" | "finished processing request" | "response failed"
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    struct LogWriter(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for LogWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("log buffer lock").extend(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for LogBuffer {
        type Writer = LogWriter;

        fn make_writer(&'writer self) -> Self::Writer {
            LogWriter(self.0.clone())
        }
    }

    #[test]
    fn trace_attributes_keep_safe_diagnostics_and_drop_private_values() {
        let attributes = sanitize_attributes(vec![
            KeyValue::new("error.type", "provider_timeout"),
            KeyValue::new("http.response.status_code", 503_i64),
            KeyValue::new("anarlog.stt.provider.name", "deepgram"),
            KeyValue::new("http.route", "/v1/transcribe/{id}"),
            KeyValue::new("user.id", "user_123"),
            KeyValue::new("error.message", "Patient Jane Doe has diabetes"),
            KeyValue::new("url.full", "https://example.com/note?token=secret"),
            KeyValue::new("component", "Jane Doe"),
        ]);

        let keys = attributes
            .iter()
            .map(|attribute| attribute.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec![
                "error.type",
                "http.response.status_code",
                "anarlog.stt.provider.name",
                "http.route"
            ]
        );
    }

    #[test]
    fn trace_names_reject_free_text_and_identifiers() {
        assert_eq!(
            sanitize_telemetry_name("transcribe.request", "operation"),
            "transcribe.request"
        );
        assert_eq!(
            sanitize_telemetry_name("transcribe patient@example.com", "operation"),
            "operation"
        );
        assert_eq!(
            sanitize_telemetry_name("sessions/019c1234-abcd-7000-8000-123456789abc", "operation"),
            "operation"
        );
        assert_eq!(
            sanitize_span_name("GET /v1/meetings/{meeting_id}"),
            "GET /v1/meetings/{meeting_id}"
        );
        assert_eq!(sanitize_span_name("GET /v1/meetings/12345"), "operation");
        assert!(is_safe_event_message("finished processing request"));
        assert!(!is_safe_event_message("failed for patient Jane Doe"));
    }

    #[test]
    fn stdout_events_keep_safe_operational_diagnostics() {
        let buffer = LogBuffer::default();
        let subscriber = tracing_subscriber::fmt()
            .event_format(SafeEventFormatter)
            .with_writer(buffer.clone())
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(
                error.type = "upstream_timeout",
                secret_token = "private-token",
                status = 503,
                "api_error_response"
            );
        });

        let output = String::from_utf8(buffer.0.lock().expect("log buffer lock").clone())
            .expect("utf-8 log output");
        assert!(output.contains("api_error_response"));
        assert!(
            output
                .split_whitespace()
                .next()
                .is_some_and(|timestamp| timestamp.contains('T'))
        );
        assert!(output.contains("error.type=upstream_timeout"));
        assert!(output.contains("status=503"));
        assert!(!output.contains("private-token"));
    }

    #[test]
    fn stdout_events_keep_safe_span_context_without_span_fields() {
        let buffer = LogBuffer::default();
        let subscriber = tracing_subscriber::fmt()
            .event_format(SafeEventFormatter)
            .with_writer(buffer.clone())
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("http_request", patient = "Jane Doe");
            let _entered = span.enter();
            tracing::info!("started processing request");
        });

        let output = String::from_utf8(buffer.0.lock().expect("log buffer lock").clone())
            .expect("utf-8 log output");
        assert!(output.contains("http_request:"));
        assert!(!output.contains("Jane Doe"));
    }

    #[test]
    fn trace_status_drops_user_errors_and_scrubs_server_descriptions() {
        let mut user_error = Status::error("no quota for this API key");
        sanitize_span_status(&mut user_error, false);
        assert_eq!(user_error, Status::Unset);

        let mut server_error = Status::error("failed to process Jane Doe's transcript");
        sanitize_span_status(&mut server_error, false);
        assert_eq!(server_error, Status::error("operation_failed"));
    }
}

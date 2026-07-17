use http::{HeaderMap, HeaderName, HeaderValue};
use opentelemetry::{
    Context, global,
    propagation::{Extractor, Injector},
    trace::TraceContextExt,
};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use reqwest::RequestBuilder;
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub fn install_trace_context_propagator() {
    global::set_text_map_propagator(TraceContextPropagator::new());
}

pub fn set_remote_parent(span: &tracing::Span, headers: &HeaderMap) {
    let parent_context = extract_remote_context(headers);
    if parent_context.span().span_context().is_valid() {
        span.set_parent(parent_context);
    }
}

pub struct TraceIdentifiers {
    pub span_id: String,
    pub trace_id: String,
}

pub fn span_identifiers(span: &tracing::Span) -> Option<TraceIdentifiers> {
    let span_context = span.context().span().span_context().clone();
    if !span_context.is_valid() {
        return None;
    }

    Some(TraceIdentifiers {
        trace_id: span_context.trace_id().to_string(),
        span_id: span_context.span_id().to_string(),
    })
}

pub fn mark_span_as_error(span: &tracing::Span, error_type: &str) {
    span.record("otel.status_code", "ERROR");
    span.record("error.type", error_type);
}

pub fn mark_current_span_as_error(error_type: &str) {
    mark_span_as_error(&tracing::Span::current(), error_type);
}

pub fn with_current_trace_context(builder: RequestBuilder) -> RequestBuilder {
    with_trace_context(builder, &tracing::Span::current().context())
}

pub fn with_trace_context(mut builder: RequestBuilder, context: &Context) -> RequestBuilder {
    let mut carrier = TraceHeaderCarrier::default();
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(context, &mut carrier);
    });

    for (key, value) in carrier.0 {
        builder = builder.header(key, value);
    }

    builder
}

pub fn inject_current_trace_context(headers: &mut HeaderMap) {
    inject_trace_context(headers, &tracing::Span::current().context());
}

pub fn inject_trace_context(headers: &mut HeaderMap, context: &Context) {
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(context, &mut HeaderMapInjector(headers));
    });
}

fn extract_remote_context(headers: &HeaderMap) -> Context {
    global::get_text_map_propagator(|propagator| propagator.extract(&HeaderMapExtractor(headers)))
}

#[derive(Default)]
struct TraceHeaderCarrier(Vec<(String, String)>);

impl Injector for TraceHeaderCarrier {
    fn set(&mut self, key: &str, value: String) {
        self.0.push((key.to_string(), value));
    }
}

struct HeaderMapInjector<'a>(&'a mut HeaderMap);

impl Injector for HeaderMapInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        let Ok(name) = HeaderName::from_bytes(key.as_bytes()) else {
            return;
        };
        let Ok(value) = HeaderValue::from_str(&value) else {
            return;
        };
        self.0.insert(name, value);
    }
}

struct HeaderMapExtractor<'a>(&'a HeaderMap);

impl Extractor for HeaderMapExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(HeaderName::as_str).collect()
    }
}

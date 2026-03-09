use std::collections::BTreeMap;
use std::time::Instant;

use async_stream::stream;
use axum::{body::Body, response::Response};
use futures_util::StreamExt;

use crate::analytics::GenerationEvent;

use super::{AnalyticsContext, AppState, report_with_cost};

pub(super) async fn handle_stream_response(
    state: AppState,
    response: reqwest::Response,
    start_time: Instant,
    analytics_ctx: AnalyticsContext,
) -> Response {
    let status = response.status();
    let http_status = status.as_u16();
    let latency_ms = start_time.elapsed().as_millis();
    let span = tracing::Span::current();
    let analytics = state.config.analytics.clone();
    let api_key = state.config.api_key.clone();
    let client = state.client.clone();
    let provider = state.config.provider.clone();

    span.record("http.response.status_code", http_status as i64);

    tracing::info!(
        http.response.status_code = %http_status,
        hyprnote.gen_ai.request.streaming = true,
        hyprnote.duration_ms = %latency_ms,
        "llm_completion_stream_started"
    );

    sentry::configure_scope(|scope| {
        scope.set_tag("http.response.status_code", http_status.to_string());

        let mut ctx = BTreeMap::new();
        ctx.insert("http.response.status_code".into(), http_status.into());
        ctx.insert("hyprnote.duration_ms".into(), (latency_ms as u64).into());
        scope.set_context("gen_ai.response", sentry::protocol::Context::Other(ctx));
    });

    let upstream = response.bytes_stream();
    let stream_span = span.clone();

    let output_stream = stream! {
        let mut accumulator = crate::provider::StreamAccumulator::new();

        futures_util::pin_mut!(upstream);

        while let Some(chunk_result) = upstream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    if analytics.is_some() {
                        provider.parse_stream_chunk(&chunk, &mut accumulator);
                    }
                    yield Ok::<_, std::io::Error>(chunk);
                }
                Err(e) => {
                    yield Err(std::io::Error::other(e));
                    break;
                }
            }
        }

        if let Some(generation_id) = accumulator.generation_id {
                stream_span.record("gen_ai.response.id", generation_id.as_str());
                if let Some(model) = accumulator.model.as_deref() {
                    stream_span.record("gen_ai.response.model", model);
                }
                stream_span.record("gen_ai.usage.input_tokens", accumulator.input_tokens as i64);
                stream_span.record("gen_ai.usage.output_tokens", accumulator.output_tokens as i64);
            if let Some(analytics) = analytics {
                let event = GenerationEvent {
                    fingerprint: analytics_ctx.fingerprint,
                    user_id: analytics_ctx.user_id,
                    generation_id,
                    model: accumulator.model.unwrap_or_default(),
                    input_tokens: accumulator.input_tokens,
                    output_tokens: accumulator.output_tokens,
                    latency: start_time.elapsed().as_secs_f64(),
                    http_status,
                    total_cost: None,
                    provider_name: provider.name().to_string(),
                    base_url: provider.base_url().to_string(),
                };
                report_with_cost(&*analytics, &*provider, &client, &api_key, event).await;
            }
        }
    };

    let body = Body::from_stream(output_stream);
    Response::builder()
        .status(status)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .body(body)
        .unwrap()
}

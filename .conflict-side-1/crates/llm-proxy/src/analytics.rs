use hypr_analytics::{AnalyticsClient, AnalyticsPayload};

#[derive(Debug, Clone)]
pub struct GenerationEvent {
    pub fingerprint: Option<String>,
    pub user_id: Option<String>,
    pub generation_id: String,
    pub model: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub latency: f64,
    pub http_status: u16,
    pub total_cost: Option<f64>,
    pub provider_name: String,
    pub base_url: String,
}

pub trait AnalyticsReporter: Send + Sync {
    fn report_generation(
        &self,
        event: GenerationEvent,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>>;
}

impl AnalyticsReporter for AnalyticsClient {
    fn report_generation(
        &self,
        event: GenerationEvent,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let payload = AnalyticsPayload::builder("$ai_generation")
                .with("$ai_provider", event.provider_name.clone())
                .with("$ai_model", event.model.clone())
                .with("$ai_input_tokens", event.input_tokens)
                .with("$ai_output_tokens", event.output_tokens)
                .with("$ai_latency", event.latency)
                .with("$ai_trace_id", event.generation_id.clone())
                .with("$ai_http_status", event.http_status)
                .with("$ai_base_url", event.base_url.clone());

            let payload = if let Some(cost) = event.total_cost {
                payload.with("$ai_total_cost_usd", cost)
            } else {
                payload
            };

            let payload = if let Some(user_id) = &event.user_id {
                payload.with("user_id", user_id.clone())
            } else {
                payload
            };

            let distinct_id = event.fingerprint.unwrap_or_else(|| {
                tracing::warn!(
                    gen_ai.response.id = %event.generation_id,
                    "device_fingerprint missing, falling back to generation_id for distinct_id"
                );
                event.generation_id.clone()
            });
            if let Err(e) = self.event(distinct_id, payload.build()).await {
                tracing::warn!("analytics event error: {e}");
            }
        })
    }
}

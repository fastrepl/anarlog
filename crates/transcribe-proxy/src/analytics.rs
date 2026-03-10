use std::time::Duration;

use hypr_analytics::{AnalyticsClient, AnalyticsPayload};

#[derive(Debug, Clone)]
pub struct SttEvent {
    pub fingerprint: Option<String>,
    pub user_id: Option<String>,
    pub provider: String,
    pub duration: Duration,
}

pub trait SttAnalyticsReporter: Send + Sync {
    fn report_stt(
        &self,
        event: SttEvent,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>>;
}

impl SttAnalyticsReporter for AnalyticsClient {
    fn report_stt(
        &self,
        event: SttEvent,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let payload = AnalyticsPayload::builder("$stt_request")
                .with("$stt_provider", event.provider.clone())
                .with("$stt_duration", event.duration.as_secs_f64());

            let payload = if let Some(user_id) = &event.user_id {
                payload.with("user_id", user_id.clone())
            } else {
                payload
            };

            let distinct_id = event.fingerprint.unwrap_or_else(|| {
                let fallback_id = uuid::Uuid::new_v4().to_string();
                tracing::warn!(
                    hyprnote.analytics.fallback_distinct_id = %fallback_id,
                    hyprnote.stt.provider.name = %event.provider,
                    "device_fingerprint missing, falling back to random UUID for distinct_id"
                );
                fallback_id
            });
            if let Err(e) = self.event(distinct_id, payload.build()).await {
                tracing::warn!("analytics event error: {e}");
            }
        })
    }
}

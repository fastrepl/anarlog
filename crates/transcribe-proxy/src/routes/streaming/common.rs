use std::pin::Pin;
use std::sync::Arc;

use owhisper_client::Provider;

use crate::config::SttProxyConfig;
use crate::provider_selector::SelectedProvider;
use crate::query_params::QueryParams;
use crate::relay::{OnCloseCallback, WebSocketProxy};

pub enum ProxyBuildError {
    SessionInitFailed(String),
    ProxyError(crate::ProxyError),
}

impl From<crate::ProxyError> for ProxyBuildError {
    fn from(e: crate::ProxyError) -> Self {
        ProxyBuildError::ProxyError(e)
    }
}

pub fn parse_param<T: std::str::FromStr>(params: &QueryParams, key: &str, default: T) -> T {
    params
        .get_first(key)
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

macro_rules! finalize_proxy_builder {
    ($builder:expr, $provider:expr, $config:expr) => {
        finalize_proxy_builder!(
            $builder,
            $provider,
            $config,
            super::AnalyticsContext {
                fingerprint: None,
                user_id: None,
            }
        )
    };
    ($builder:expr, $provider:expr, $config:expr, $analytics_ctx:expr) => {
        match &$config.analytics {
            Some(analytics) => {
                let analytics = analytics.clone();
                let provider_name = format!("{:?}", $provider).to_lowercase();
                let analytics_ctx: super::AnalyticsContext = $analytics_ctx;
                $builder
                    .on_close(move |duration| {
                        let analytics = analytics.clone();
                        let provider_name = provider_name.clone();
                        let fingerprint = analytics_ctx.fingerprint.clone();
                        let user_id = analytics_ctx.user_id.clone();
                        async move {
                            analytics
                                .report_stt($crate::analytics::SttEvent {
                                    fingerprint,
                                    user_id,
                                    provider: provider_name,
                                    duration,
                                })
                                .await;
                        }
                    })
                    .build()
            }
            None => $builder.build(),
        }
    };
}

pub(super) use finalize_proxy_builder;

pub fn build_on_close_callback(
    config: &SttProxyConfig,
    provider: Provider,
    analytics_ctx: &super::AnalyticsContext,
) -> Option<OnCloseCallback> {
    let analytics = config.analytics.as_ref()?;
    let analytics = analytics.clone();
    let provider_name = format!("{:?}", provider).to_lowercase();
    let fingerprint = analytics_ctx.fingerprint.clone();
    let user_id = analytics_ctx.user_id.clone();

    Some(Arc::new(move |duration: std::time::Duration| {
        let analytics = analytics.clone();
        let provider_name = provider_name.clone();
        let fingerprint = fingerprint.clone();
        let user_id = user_id.clone();
        Box::pin(async move {
            analytics
                .report_stt(crate::analytics::SttEvent {
                    fingerprint,
                    user_id,
                    provider: provider_name,
                    duration,
                })
                .await;
        }) as Pin<Box<dyn std::future::Future<Output = ()> + Send>>
    }))
}

pub fn build_proxy_with_url(
    selected: &SelectedProvider,
    upstream_url: &str,
    config: &SttProxyConfig,
    analytics_ctx: super::AnalyticsContext,
) -> Result<WebSocketProxy, crate::ProxyError> {
    let provider = selected.provider();
    let builder = WebSocketProxy::builder()
        .upstream_url(upstream_url)
        .connect_timeout(config.connect_timeout)
        .control_message_types(provider.control_message_types())
        .apply_auth(selected);

    finalize_proxy_builder!(builder, provider, config, analytics_ctx)
}

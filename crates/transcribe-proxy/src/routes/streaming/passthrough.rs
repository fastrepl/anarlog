use owhisper_client::Auth;

use crate::config::SttProxyConfig;
use crate::provider_selector::SelectedProvider;
use crate::query_params::{QueryParams, QueryValue};
use crate::relay::WebSocketProxy;
use crate::routes::AppState;

use super::AnalyticsContext;
use super::common::{ProxyBuildError, build_proxy_with_url, finalize_proxy_builder};
use super::session::init_session;

fn build_relay(
    selected: &SelectedProvider,
    client_params: &QueryParams,
    config: &SttProxyConfig,
    analytics_ctx: AnalyticsContext,
) -> Result<WebSocketProxy, crate::ProxyError> {
    let provider = selected.provider();
    let ws_url = provider.default_ws_url();

    let mut upstream_url: url::Url = ws_url
        .parse()
        .map_err(|e| crate::ProxyError::InvalidRequest(format!("{}", e)))?;

    {
        let mut query = upstream_url.query_pairs_mut();
        for (key, value) in client_params.iter() {
            match value {
                QueryValue::Single(v) => {
                    query.append_pair(key, v);
                }
                QueryValue::Multi(values) => {
                    for v in values {
                        query.append_pair(key, v);
                    }
                }
            }
        }
    }

    let builder = WebSocketProxy::builder()
        .upstream_url(upstream_url.as_str())
        .connect_timeout(config.connect_timeout)
        .control_message_types(provider.control_message_types())
        .apply_auth(selected);

    finalize_proxy_builder!(builder, provider, config, analytics_ctx)
}

pub async fn build_proxy(
    state: &AppState,
    selected: &SelectedProvider,
    params: &QueryParams,
    analytics_ctx: AnalyticsContext,
) -> Result<WebSocketProxy, ProxyBuildError> {
    let provider = selected.provider();

    if let Some(custom_url) = selected.upstream_url() {
        return Ok(build_proxy_with_url(
            selected,
            custom_url,
            &state.config,
            analytics_ctx,
        )?);
    }

    match provider.auth() {
        Auth::SessionInit { header_name } => {
            let url = init_session(state, selected, header_name, params)
                .await
                .map_err(ProxyBuildError::SessionInitFailed)?;
            Ok(build_proxy_with_url(
                selected,
                &url,
                &state.config,
                analytics_ctx,
            )?)
        }
        _ => Ok(build_relay(selected, params, &state.config, analytics_ctx)?),
    }
}

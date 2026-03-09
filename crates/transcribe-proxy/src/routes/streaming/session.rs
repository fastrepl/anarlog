use owhisper_client::Provider;

use crate::provider_selector::SelectedProvider;
use crate::query_params::QueryParams;
use crate::routes::AppState;

use super::common::parse_param;

#[derive(serde::Deserialize)]
pub struct InitResponse {
    pub id: String,
    pub url: String,
}

pub fn build_session_config(
    provider: Provider,
    params: &QueryParams,
) -> Result<serde_json::Value, String> {
    let sample_rate: u32 = parse_param(params, "sample_rate", 16000);
    let channels: u8 = parse_param(params, "channels", 1);
    provider
        .session_init_config(sample_rate, channels)
        .ok_or_else(|| format!("{:?} does not support session init config", provider))
}

#[tracing::instrument(
    name = "stt.session.init",
    skip(state, selected, params),
    fields(
        hyprnote.subsystem = "stt",
        hyprnote.stt.provider.name = ?selected.provider()
    )
)]
pub async fn init_session(
    state: &AppState,
    selected: &SelectedProvider,
    header_name: &'static str,
    params: &QueryParams,
) -> Result<String, String> {
    let provider = selected.provider();
    let init_url = provider
        .default_api_url()
        .ok_or_else(|| format!("{:?} does not support session init", provider))?;

    let config = build_session_config(provider, params)?;

    let resp = hypr_observability::with_current_trace_context(
        state
            .client
            .post(init_url)
            .header(header_name, selected.api_key())
            .header("Content-Type", "application/json"),
    )
    .json(&config)
    .send()
    .await
    .map_err(|e| format!("session init request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("session init failed: {} - {}", status, body));
    }

    let init: InitResponse = resp
        .json()
        .await
        .map_err(|e| format!("session init parse failed: {}", e))?;

    tracing::debug!(
        hyprnote.stt.session.id = %init.id,
        hyprnote.stt.provider.name = ?provider,
        "session_initialized"
    );

    Ok(init.url)
}

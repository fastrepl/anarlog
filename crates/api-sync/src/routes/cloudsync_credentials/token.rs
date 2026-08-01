use chrono::{SecondsFormat, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    error::{Result, SyncError},
    state::AppState,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct E2eeCreateTokenRequest<'a> {
    pub(super) name: &'static str,
    pub(super) user_id: &'a str,
    pub(super) expires_at: &'a str,
    pub(super) attributes: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LegacyCreateTokenRequest<'a> {
    pub(super) name: &'static str,
    pub(super) user_id: &'a str,
    pub(super) expires_at: &'a str,
}

#[derive(Deserialize)]
struct CreateTokenEnvelope {
    data: CreateTokenResponse,
}

#[derive(Deserialize)]
struct CreateTokenResponse {
    token: String,
}

pub(super) fn token_expiry(token_ttl_seconds: u64) -> Result<String> {
    let ttl = i64::try_from(token_ttl_seconds)
        .map_err(|_| SyncError::Internal("CloudSync token TTL is too large".to_string()))?;
    let ttl = TimeDelta::try_seconds(ttl)
        .ok_or_else(|| SyncError::Internal("CloudSync token TTL is too large".to_string()))?;
    Utc::now()
        .checked_add_signed(ttl)
        .ok_or_else(|| SyncError::Internal("CloudSync token expiry is invalid".to_string()))
        .map(|expiry| expiry.to_rfc3339_opts(SecondsFormat::Secs, true))
}

pub(super) async fn mint_cloudsync_token(
    state: &AppState,
    request: &impl Serialize,
) -> Result<String> {
    let response = state
        .client
        .post(format!("{}/v2/tokens", state.config.project_url))
        .bearer_auth(&state.config.token_issuer_api_key)
        .json(request)
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "SQLite Cloud token request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "SQLite Cloud token request was rejected");
        return Err(SyncError::Upstream);
    }
    let response: CreateTokenEnvelope = response.json().await.map_err(|error| {
        tracing::warn!(%error, "SQLite Cloud token response was invalid");
        SyncError::Upstream
    })?;
    if response.data.token.trim().is_empty() {
        return Err(SyncError::Upstream);
    }
    Ok(response.data.token)
}

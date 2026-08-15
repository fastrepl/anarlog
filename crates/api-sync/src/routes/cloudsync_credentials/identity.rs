use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use super::SUPABASE_REQUEST_TIMEOUT;
use crate::{
    error::{Result, SyncError},
    state::ReplicaState,
};

pub(in crate::routes) const DEVICE_FINGERPRINT_HEADER: &str = "x-device-fingerprint";
pub(in crate::routes) const DEVICE_NAME_HEADER: &str = "x-anarlog-device-name";
pub(in crate::routes) const E2EE_MEMBER_PUBLIC_KEY_HEADER: &str =
    "x-anarlog-e2ee-member-public-key";
const MAX_DEVICE_NAME_BYTES: usize = 128;

#[derive(Serialize)]
struct ClaimE2eeKeyRpcRequest<'a> {
    p_actor_user_id: &'a str,
    p_key_id: &'a str,
}

#[derive(Deserialize)]
struct E2eeKeyIdRow {
    key_id: String,
}

#[derive(Serialize)]
struct PublishE2eeMemberIdentityRpcRequest<'a> {
    p_public_key: &'a str,
}

#[derive(Deserialize)]
struct E2eeMemberIdentityRow {
    public_key: String,
}

#[derive(Serialize)]
struct ClaimSyncDeviceRpcRequest<'a> {
    p_actor_user_id: &'a str,
    p_device_fingerprint: &'a str,
    p_device_name: Option<&'a str>,
}

#[derive(Deserialize)]
struct SyncDeviceClaimRow {
    allowed: bool,
    device_count: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SyncDeviceRow {
    pub device_fingerprint: String,
    pub device_name: Option<String>,
    pub created_at: String,
    pub last_seen_at: String,
}

pub(super) async fn list_sync_devices(
    state: &ReplicaState,
    account_user_id: &str,
) -> Result<Vec<SyncDeviceRow>> {
    let response = state
        .client
        .get(format!(
            "{}/rest/v1/sync_devices",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .query(&[
            ("user_id", format!("eq.{account_user_id}")),
            (
                "select",
                "device_fingerprint,device_name,created_at,last_seen_at".to_string(),
            ),
            ("order", "last_seen_at.desc".to_string()),
        ])
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|_| SyncError::Upstream)?;
    if !response.status().is_success() {
        return Err(SyncError::Upstream);
    }
    response.json().await.map_err(|_| SyncError::Upstream)
}

pub(super) async fn remove_sync_device(
    state: &ReplicaState,
    account_user_id: &str,
    fingerprint: &str,
) -> Result<()> {
    if !is_valid_device_fingerprint(fingerprint) {
        return Err(SyncError::BadRequest(
            "Sync device identity is invalid".to_string(),
        ));
    }
    let response = state
        .client
        .delete(format!(
            "{}/rest/v1/sync_devices",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .query(&[
            ("user_id", format!("eq.{account_user_id}")),
            ("device_fingerprint", format!("eq.{fingerprint}")),
        ])
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|_| SyncError::Upstream)?;
    if !response.status().is_success() {
        return Err(SyncError::Upstream);
    }
    Ok(())
}

fn is_valid_device_fingerprint(fingerprint: &str) -> bool {
    (8..=128).contains(&fingerprint.len())
        && fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub(super) async fn claim_sync_device(
    state: &ReplicaState,
    account_user_id: &str,
    headers: &HeaderMap,
) -> Result<()> {
    // Older desktop builds do not identify themselves; the limit only
    // applies once the client sends its fingerprint.
    let Some(fingerprint) = headers
        .get(DEVICE_FINGERPRINT_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|fingerprint| !fingerprint.is_empty())
    else {
        return Ok(());
    };
    if !is_valid_device_fingerprint(fingerprint) {
        tracing::warn!("sync device fingerprint failed validation; skipping device claim");
        return Ok(());
    }

    let device_name = headers
        .get(DEVICE_NAME_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|name| !name.is_empty() && name.len() <= MAX_DEVICE_NAME_BYTES);

    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/claim_sync_device",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .json(&ClaimSyncDeviceRpcRequest {
            p_actor_user_id: account_user_id,
            p_device_fingerprint: fingerprint,
            p_device_name: device_name,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase sync device claim failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "Supabase sync device claim was rejected");
        return Err(SyncError::Upstream);
    }
    let rows = response
        .json::<Vec<SyncDeviceClaimRow>>()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase sync device claim response was invalid");
            SyncError::Upstream
        })?;
    let Some(row) = rows.first() else {
        tracing::warn!("Supabase sync device claim returned no rows");
        return Err(SyncError::Upstream);
    };
    if !row.allowed {
        tracing::info!(
            device_count = row.device_count,
            "CloudSync device limit reached"
        );
        return Err(SyncError::SyncDeviceLimitReached);
    }
    Ok(())
}

pub(super) async fn claim_personal_e2ee_key(
    state: &ReplicaState,
    account_user_id: &str,
    requested_key_id: &str,
) -> Result<String> {
    if !is_valid_e2ee_key_id(requested_key_id) {
        return Err(SyncError::BadRequest(
            "E2EE key identity is invalid".to_string(),
        ));
    }

    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/claim_personal_workspace_e2ee_key",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .json(&ClaimE2eeKeyRpcRequest {
            p_actor_user_id: account_user_id,
            p_key_id: requested_key_id,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase E2EE identity request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "Supabase E2EE identity request was rejected");
        return Err(SyncError::Upstream);
    }
    let mut rows = response
        .json::<Vec<E2eeKeyIdRow>>()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase E2EE identity response was invalid");
            SyncError::Upstream
        })?;
    if rows.len() != 1 || !is_valid_e2ee_key_id(&rows[0].key_id) {
        tracing::warn!(
            row_count = rows.len(),
            "Supabase E2EE identity response failed validation"
        );
        return Err(SyncError::Upstream);
    }
    let key_id = rows.pop().expect("row count was checked").key_id;
    if key_id != requested_key_id {
        return Err(SyncError::E2eeKeyMismatch);
    }
    Ok(key_id)
}

pub(super) async fn publish_e2ee_member_identity(
    state: &ReplicaState,
    access_token: &str,
    public_key: &str,
) -> Result<()> {
    if !is_valid_e2ee_member_public_key(public_key) {
        return Err(SyncError::BadRequest(
            "E2EE member identity is invalid".to_string(),
        ));
    }

    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/publish_e2ee_member_identity",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_anon_key)
        .bearer_auth(access_token)
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .json(&PublishE2eeMemberIdentityRpcRequest {
            p_public_key: public_key,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase E2EE member identity request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(
            status = %response.status(),
            "Supabase E2EE member identity request was rejected"
        );
        return Err(SyncError::Upstream);
    }
    let rows = response
        .json::<Vec<E2eeMemberIdentityRow>>()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase E2EE member identity response was invalid");
            SyncError::Upstream
        })?;
    if rows.len() != 1 || rows[0].public_key != public_key {
        tracing::warn!(
            row_count = rows.len(),
            "Supabase E2EE member identity response failed validation"
        );
        return Err(SyncError::Upstream);
    }
    Ok(())
}

pub(super) fn is_valid_e2ee_key_id(key_id: &str) -> bool {
    key_id.len() == 22
        && key_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn is_valid_e2ee_member_public_key(public_key: &str) -> bool {
    public_key.len() == 43
        && public_key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

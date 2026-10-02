use anlg_api_auth::AuthContext;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};

use super::{
    SUPABASE_REQUEST_TIMEOUT,
    identity::{DEVICE_FINGERPRINT_HEADER, is_valid_device_fingerprint},
};
use crate::{
    error::{Result, SyncError},
    state::ReplicaState,
};

#[derive(Clone, Copy, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum MeetingDeviceIntent {
    Present,
    Claim,
    Release,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeetingDeviceHeartbeatRequest {
    intent: MeetingDeviceIntent,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MeetingDevice {
    #[serde(alias = "device_fingerprint")]
    device_fingerprint: String,
    #[serde(alias = "device_name")]
    device_name: Option<String>,
    #[serde(alias = "is_primary")]
    primary: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MeetingDevicesResponse {
    devices: Vec<MeetingDevice>,
}

#[derive(Serialize)]
struct HeartbeatMeetingDeviceRpcRequest<'a> {
    p_actor_user_id: &'a str,
    p_meeting_key: &'a str,
    p_device_fingerprint: &'a str,
    p_intent: MeetingDeviceIntent,
}

fn is_valid_meeting_key(key: &str) -> bool {
    (16..=128).contains(&key.len())
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[utoipa::path(
    post,
    path = "/meetings/{meeting_key}/devices",
    tag = "sync",
    params(
        ("meeting_key" = String, Path, description = "Opaque key derived from the calendar event"),
        ("x-device-fingerprint" = String, Header, description = "Fingerprint of the calling device")
    ),
    request_body = MeetingDeviceHeartbeatRequest,
    responses(
        (status = 200, description = "Devices present for the meeting", body = MeetingDevicesResponse),
        (status = 400, description = "Invalid meeting key or device fingerprint"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription required"),
        (status = 404, description = "The calling device is not a registered sync device"),
        (status = 502, description = "Device service unavailable")
    )
)]
pub(super) async fn heartbeat_meeting_device(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    Path(meeting_key): Path<String>,
    headers: HeaderMap,
    Json(request): Json<MeetingDeviceHeartbeatRequest>,
) -> Result<Json<MeetingDevicesResponse>> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    if !is_valid_meeting_key(&meeting_key) {
        return Err(SyncError::BadRequest("Invalid meeting key".to_string()));
    }
    let fingerprint = headers
        .get(DEVICE_FINGERPRINT_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|fingerprint| is_valid_device_fingerprint(fingerprint))
        .ok_or_else(|| SyncError::BadRequest("Invalid device fingerprint".to_string()))?;

    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/heartbeat_meeting_device",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .json(&HeartbeatMeetingDeviceRpcRequest {
            p_actor_user_id: &auth.claims.sub,
            p_meeting_key: &meeting_key,
            p_device_fingerprint: fingerprint,
            p_intent: request.intent,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "meeting device heartbeat failed");
            SyncError::Upstream
        })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        if body.contains("\"42501\"") {
            return Err(SyncError::MeetingDeviceNotRegistered);
        }
        tracing::error!(%status, "meeting device heartbeat was rejected");
        return Err(if status == StatusCode::BAD_REQUEST {
            SyncError::BadRequest("Invalid meeting device request".to_string())
        } else {
            SyncError::Upstream
        });
    }

    let devices = response
        .json::<Vec<MeetingDevice>>()
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "meeting device heartbeat returned an invalid body");
            SyncError::Upstream
        })?;
    Ok(Json(MeetingDevicesResponse { devices }))
}

use anlg_api_auth::AuthContext;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    SUPABASE_REQUEST_TIMEOUT,
    identity::{DEVICE_FINGERPRINT_HEADER, DEVICE_NAME_HEADER, is_valid_device_fingerprint},
};
use crate::{
    error::{Result, SyncError},
    state::ReplicaState,
};

const MAX_DEVICE_NAME_BYTES: usize = 128;
const MAX_ENROLLMENT_CIPHERTEXT_BYTES: usize = 2_048;
const MAX_SYNC_DEVICES: i64 = 5;

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct E2eeDeviceEnrollmentPackage {
    pub ephemeral_public_key: String,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegisterE2eeDeviceEnrollmentRequest {
    public_key: String,
    replace_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConsumeE2eeDeviceEnrollmentRequest {
    public_key: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum E2eeDeviceEnrollmentStatus {
    Pending,
    Sealed,
    Consumed,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisterE2eeDeviceEnrollmentResponse {
    request_id: String,
    expires_at: String,
    status: E2eeDeviceEnrollmentStatus,
    package: Option<E2eeDeviceEnrollmentPackage>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct E2eeDeviceEnrollmentSummary {
    request_id: String,
    device_fingerprint: String,
    device_name: Option<String>,
    public_key: String,
    created_at: String,
    expires_at: String,
    status: E2eeDeviceEnrollmentStatus,
}

#[derive(Serialize)]
struct RegisterEnrollmentRpcRequest<'a> {
    p_actor_user_id: &'a str,
    p_device_fingerprint: &'a str,
    p_device_name: Option<&'a str>,
    p_recipient_public_key: &'a str,
    p_replace_device_fingerprint: Option<&'a str>,
}

#[derive(Deserialize)]
struct RegisterEnrollmentRpcRow {
    allowed: bool,
    requires_existing_key: bool,
    request_id: Option<String>,
    expires_at: Option<String>,
    enrollment_status: Option<String>,
    ephemeral_public_key: Option<String>,
    nonce: Option<String>,
    ciphertext: Option<String>,
    device_count: i64,
}

#[derive(Serialize)]
struct SealEnrollmentRpcRequest<'a> {
    p_actor_user_id: &'a str,
    p_request_id: &'a str,
    p_ephemeral_public_key: &'a str,
    p_nonce: &'a str,
    p_ciphertext: &'a str,
}

#[derive(Deserialize)]
struct SealEnrollmentRpcRow {
    result: String,
}

#[derive(Serialize)]
struct ConsumeEnrollmentRpcRequest<'a> {
    p_actor_user_id: &'a str,
    p_request_id: &'a str,
    p_device_fingerprint: &'a str,
    p_recipient_public_key: &'a str,
}

#[derive(Deserialize)]
struct ConsumeEnrollmentRpcRow {
    consumed: bool,
}

#[derive(Deserialize)]
struct EnrollmentRow {
    id: String,
    device_fingerprint: String,
    device_name: Option<String>,
    recipient_public_key: String,
    created_at: String,
    expires_at: String,
    sealed_at: Option<String>,
    consumed_at: Option<String>,
}

#[utoipa::path(
    post,
    path = "/e2ee/device-enrollments",
    tag = "sync",
    params(
        ("x-device-fingerprint" = String, Header, description = "Current device fingerprint"),
        ("x-anarlog-device-name" = Option<String>, Header, description = "Current device name")
    ),
    request_body = RegisterE2eeDeviceEnrollmentRequest,
    responses(
        (status = 200, description = "Current enrollment request", body = RegisterE2eeDeviceEnrollmentResponse),
        (status = 400, description = "Invalid device enrollment"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro or a free device slot is required"),
        (status = 409, description = "An existing device must establish encrypted sync first"),
        (status = 502, description = "Enrollment service unavailable")
    )
)]
pub(super) async fn register_e2ee_device_enrollment(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    headers: HeaderMap,
    Json(request): Json<RegisterE2eeDeviceEnrollmentRequest>,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<RegisterE2eeDeviceEnrollmentResponse>,
)> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    let (fingerprint, device_name) = required_device_headers(&headers)?;
    let response =
        register_enrollment(&state, &auth.claims.sub, fingerprint, device_name, request).await?;
    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(response),
    ))
}

#[utoipa::path(
    post,
    path = "/e2ee/device-enrollments/{request_id}/seal",
    tag = "sync",
    params(("request_id" = String, Path, description = "Enrollment request ID")),
    request_body = E2eeDeviceEnrollmentPackage,
    responses(
        (status = 204, description = "Enrollment package sealed"),
        (status = 400, description = "Invalid enrollment package"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription required"),
        (status = 404, description = "Enrollment request unavailable"),
        (status = 409, description = "Enrollment request already sealed"),
        (status = 502, description = "Enrollment service unavailable")
    )
)]
pub(super) async fn seal_e2ee_device_enrollment(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    Path(request_id): Path<String>,
    Json(package): Json<E2eeDeviceEnrollmentPackage>,
) -> Result<StatusCode> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    seal_enrollment(&state, &auth.claims.sub, &request_id, &package).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/e2ee/device-enrollments/{request_id}/consume",
    tag = "sync",
    params(
        ("request_id" = String, Path, description = "Enrollment request ID"),
        ("x-device-fingerprint" = String, Header, description = "Requesting device fingerprint")
    ),
    request_body = ConsumeE2eeDeviceEnrollmentRequest,
    responses(
        (status = 204, description = "Enrollment package consumed"),
        (status = 400, description = "Invalid enrollment acknowledgement"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription required"),
        (status = 404, description = "Enrollment request unavailable"),
        (status = 502, description = "Enrollment service unavailable")
    )
)]
pub(super) async fn consume_e2ee_device_enrollment(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
    Json(request): Json<ConsumeE2eeDeviceEnrollmentRequest>,
) -> Result<StatusCode> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    let (fingerprint, _) = required_device_headers(&headers)?;
    consume_enrollment(
        &state,
        &auth.claims.sub,
        &request_id,
        fingerprint,
        &request.public_key,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn register_enrollment(
    state: &ReplicaState,
    account_user_id: &str,
    fingerprint: &str,
    device_name: Option<&str>,
    request: RegisterE2eeDeviceEnrollmentRequest,
) -> Result<RegisterE2eeDeviceEnrollmentResponse> {
    if !is_valid_base64url(&request.public_key, 43)
        || request
            .replace_fingerprint
            .as_deref()
            .is_some_and(|value| !is_valid_device_fingerprint(value) || value == fingerprint)
    {
        return Err(SyncError::BadRequest(
            "E2EE device enrollment is invalid".to_string(),
        ));
    }
    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/register_e2ee_device_enrollment",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .json(&RegisterEnrollmentRpcRequest {
            p_actor_user_id: account_user_id,
            p_device_fingerprint: fingerprint,
            p_device_name: device_name,
            p_recipient_public_key: &request.public_key,
            p_replace_device_fingerprint: request.replace_fingerprint.as_deref(),
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase E2EE device enrollment request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(
            status = %response.status(),
            "Supabase E2EE device enrollment request was rejected"
        );
        return Err(SyncError::Upstream);
    }
    let mut rows = response
        .json::<Vec<RegisterEnrollmentRpcRow>>()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase E2EE device enrollment response was invalid");
            SyncError::Upstream
        })?;
    if rows.len() != 1 {
        return Err(SyncError::Upstream);
    }
    let row = rows.pop().expect("row count was checked");
    if row.requires_existing_key {
        return Err(SyncError::E2eeEnrollmentRequiresExistingKey);
    }
    if !row.allowed {
        if row.device_count > 0 {
            return Err(SyncError::SyncDeviceLimitReached);
        }
        return Err(SyncError::Upstream);
    }
    validate_registration_row(row)
}

fn validate_registration_row(
    row: RegisterEnrollmentRpcRow,
) -> Result<RegisterE2eeDeviceEnrollmentResponse> {
    let request_id = row.request_id.ok_or(SyncError::Upstream)?;
    validate_request_id(&request_id)?;
    let expires_at = row.expires_at.ok_or(SyncError::Upstream)?;
    let expiration = DateTime::parse_from_rfc3339(&expires_at)
        .map_err(|_| SyncError::Upstream)?
        .with_timezone(&Utc);
    if expiration <= Utc::now() || !(1..=MAX_SYNC_DEVICES).contains(&row.device_count) {
        return Err(SyncError::Upstream);
    }
    let status = parse_status(row.enrollment_status.as_deref())?;
    let package = parse_package(
        row.ephemeral_public_key,
        row.nonce,
        row.ciphertext,
        matches!(status, E2eeDeviceEnrollmentStatus::Sealed),
    )?;
    Ok(RegisterE2eeDeviceEnrollmentResponse {
        request_id,
        expires_at,
        status,
        package,
    })
}

async fn seal_enrollment(
    state: &ReplicaState,
    account_user_id: &str,
    request_id: &str,
    package: &E2eeDeviceEnrollmentPackage,
) -> Result<()> {
    validate_request_id(request_id)?;
    validate_package(package)?;
    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/seal_e2ee_device_enrollment",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .json(&SealEnrollmentRpcRequest {
            p_actor_user_id: account_user_id,
            p_request_id: request_id,
            p_ephemeral_public_key: &package.ephemeral_public_key,
            p_nonce: &package.nonce,
            p_ciphertext: &package.ciphertext,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase E2EE device enrollment seal failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        return Err(SyncError::Upstream);
    }
    let mut rows = response
        .json::<Vec<SealEnrollmentRpcRow>>()
        .await
        .map_err(|_| SyncError::Upstream)?;
    if rows.len() != 1 {
        return Err(SyncError::Upstream);
    }
    match rows.pop().expect("row count was checked").result.as_str() {
        "sealed" => Ok(()),
        "conflict" => Err(SyncError::E2eeEnrollmentConflict),
        "unavailable" => Err(SyncError::E2eeEnrollmentUnavailable),
        _ => Err(SyncError::Upstream),
    }
}

async fn consume_enrollment(
    state: &ReplicaState,
    account_user_id: &str,
    request_id: &str,
    fingerprint: &str,
    public_key: &str,
) -> Result<()> {
    validate_request_id(request_id)?;
    if !is_valid_base64url(public_key, 43) {
        return Err(SyncError::BadRequest(
            "E2EE device enrollment is invalid".to_string(),
        ));
    }
    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/consume_e2ee_device_enrollment",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .json(&ConsumeEnrollmentRpcRequest {
            p_actor_user_id: account_user_id,
            p_request_id: request_id,
            p_device_fingerprint: fingerprint,
            p_recipient_public_key: public_key,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase E2EE device enrollment acknowledgement failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        return Err(SyncError::Upstream);
    }
    let mut rows = response
        .json::<Vec<ConsumeEnrollmentRpcRow>>()
        .await
        .map_err(|_| SyncError::Upstream)?;
    if rows.len() != 1 {
        return Err(SyncError::Upstream);
    }
    if rows.pop().expect("row count was checked").consumed {
        Ok(())
    } else {
        Err(SyncError::E2eeEnrollmentUnavailable)
    }
}

pub(super) async fn list_e2ee_device_enrollments(
    state: &ReplicaState,
    account_user_id: &str,
) -> Result<Vec<E2eeDeviceEnrollmentSummary>> {
    let response = state
        .client
        .get(format!(
            "{}/rest/v1/e2ee_device_enrollment_requests",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .query(&[
            ("user_id", format!("eq.{account_user_id}")),
            ("consumed_at", "is.null".to_string()),
            (
                "select",
                "id,device_fingerprint,device_name,recipient_public_key,created_at,expires_at,sealed_at,consumed_at".to_string(),
            ),
            ("order", "created_at.desc".to_string()),
        ])
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|_| SyncError::Upstream)?;
    if !response.status().is_success() {
        return Err(SyncError::Upstream);
    }
    let rows = response
        .json::<Vec<EnrollmentRow>>()
        .await
        .map_err(|_| SyncError::Upstream)?;
    validate_enrollment_rows(rows)
}

fn validate_enrollment_rows(rows: Vec<EnrollmentRow>) -> Result<Vec<E2eeDeviceEnrollmentSummary>> {
    if rows.len() > MAX_SYNC_DEVICES as usize {
        return Err(SyncError::Upstream);
    }
    let now = Utc::now();
    let mut enrollments = Vec::with_capacity(rows.len());
    for row in rows {
        let expires_at = DateTime::parse_from_rfc3339(&row.expires_at)
            .map_err(|_| SyncError::Upstream)?
            .with_timezone(&Utc);
        if expires_at <= now {
            continue;
        }
        validate_request_id(&row.id).map_err(|_| SyncError::Upstream)?;
        if !is_valid_device_fingerprint(&row.device_fingerprint)
            || !is_valid_base64url(&row.recipient_public_key, 43)
            || row
                .device_name
                .as_ref()
                .is_some_and(|name| name.is_empty() || name.len() > MAX_DEVICE_NAME_BYTES)
            || DateTime::parse_from_rfc3339(&row.created_at).is_err()
        {
            return Err(SyncError::Upstream);
        }
        let status = if row.consumed_at.is_some() {
            E2eeDeviceEnrollmentStatus::Consumed
        } else if row.sealed_at.is_some() {
            E2eeDeviceEnrollmentStatus::Sealed
        } else {
            E2eeDeviceEnrollmentStatus::Pending
        };
        enrollments.push(E2eeDeviceEnrollmentSummary {
            request_id: row.id,
            device_fingerprint: row.device_fingerprint,
            device_name: row.device_name,
            public_key: row.recipient_public_key,
            created_at: row.created_at,
            expires_at: row.expires_at,
            status,
        });
    }
    Ok(enrollments)
}

fn required_device_headers(headers: &HeaderMap) -> Result<(&str, Option<&str>)> {
    let fingerprint = headers
        .get(DEVICE_FINGERPRINT_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| is_valid_device_fingerprint(value))
        .ok_or_else(|| {
            SyncError::BadRequest("E2EE device enrollment identity is invalid".to_string())
        })?;
    let device_name = headers
        .get(DEVICE_NAME_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if device_name.is_some_and(|value| value.len() > MAX_DEVICE_NAME_BYTES) {
        return Err(SyncError::BadRequest(
            "E2EE device enrollment identity is invalid".to_string(),
        ));
    }
    Ok((fingerprint, device_name))
}

fn parse_status(value: Option<&str>) -> Result<E2eeDeviceEnrollmentStatus> {
    match value {
        Some("pending") => Ok(E2eeDeviceEnrollmentStatus::Pending),
        Some("sealed") => Ok(E2eeDeviceEnrollmentStatus::Sealed),
        Some("consumed") => Ok(E2eeDeviceEnrollmentStatus::Consumed),
        _ => Err(SyncError::Upstream),
    }
}

fn parse_package(
    ephemeral_public_key: Option<String>,
    nonce: Option<String>,
    ciphertext: Option<String>,
    required: bool,
) -> Result<Option<E2eeDeviceEnrollmentPackage>> {
    match (ephemeral_public_key, nonce, ciphertext) {
        (Some(ephemeral_public_key), Some(nonce), Some(ciphertext)) if required => {
            let package = E2eeDeviceEnrollmentPackage {
                ephemeral_public_key,
                nonce,
                ciphertext,
            };
            validate_package(&package).map_err(|_| SyncError::Upstream)?;
            Ok(Some(package))
        }
        (None, None, None) if !required => Ok(None),
        _ => Err(SyncError::Upstream),
    }
}

fn validate_package(package: &E2eeDeviceEnrollmentPackage) -> Result<()> {
    if !is_valid_base64url(&package.ephemeral_public_key, 43)
        || !is_valid_base64url(&package.nonce, 32)
        || package.ciphertext.len() < 64
        || package.ciphertext.len() > MAX_ENROLLMENT_CIPHERTEXT_BYTES
        || !is_base64url(&package.ciphertext)
    {
        return Err(SyncError::BadRequest(
            "E2EE device enrollment package is invalid".to_string(),
        ));
    }
    Ok(())
}

fn validate_request_id(request_id: &str) -> Result<()> {
    let parsed = Uuid::parse_str(request_id).map_err(|_| {
        SyncError::BadRequest("E2EE device enrollment request is invalid".to_string())
    })?;
    if parsed.to_string() != request_id {
        return Err(SyncError::BadRequest(
            "E2EE device enrollment request is invalid".to_string(),
        ));
    }
    Ok(())
}

fn is_valid_base64url(value: &str, length: usize) -> bool {
    value.len() == length && is_base64url(value)
}

fn is_base64url(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

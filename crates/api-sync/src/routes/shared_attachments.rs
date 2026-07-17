use std::time::Duration;

use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{HeaderValue, header},
    middleware::{self, Next},
    response::Response,
    routing::post,
};
use chrono::{SecondsFormat, TimeDelta, Utc};
use hypr_api_auth::AuthContext;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use utoipa::OpenApi;
use uuid::{Uuid, Version};

use crate::{
    error::{Result, SyncError},
    state::AppState,
};

const BUCKET: &str = "shared-note-attachments";
const RPC_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_REQUEST_BYTES: usize = 8 * 1024;
const MAX_RPC_RESPONSE_BYTES: usize = 32 * 1024;
const MAX_SIZE_BYTES: u64 = 512 * 1024 * 1024;
const UPLOAD_TTL_SECONDS: i64 = 2 * 60 * 60;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ReserveSharedAttachmentRequest {
    attachment_ref: String,
    version_ref: String,
    filename: String,
    content_type: String,
    size_bytes: u64,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReservedSharedAttachment {
    attachment_id: String,
    object_key: String,
    object_state: String,
    filename: String,
    content_type: String,
    size_bytes: u64,
    sha256: Option<String>,
    reservation_expires_at: String,
    was_created: bool,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SharedAttachmentObjectRequest {
    object_key: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct GrantSharedAttachmentUploadRequest {
    object_key: String,
    sha256: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct SharedAttachmentUploadGrant {
    attachment_id: String,
    object_key: String,
    object_state: String,
    filename: String,
    content_type: String,
    size_bytes: u64,
    sha256: String,
    upload_expires_at: Option<String>,
    upload_token: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FinalizedSharedAttachment {
    attachment_id: String,
    object_key: String,
    object_state: String,
    was_finalized: bool,
}

#[derive(Serialize)]
struct ReserveRpcRequest<'a> {
    p_share_id: &'a str,
    p_actor_user_id: &'a str,
    p_attachment_ref: &'a str,
    p_version_ref: &'a str,
    p_filename: &'a str,
    p_content_type: &'a str,
    p_size_bytes: i64,
}

#[derive(Serialize)]
struct ObjectKeyRpcRequest<'a> {
    p_share_id: &'a str,
    p_actor_user_id: &'a str,
    p_object_key: &'a str,
}

#[derive(Serialize)]
struct MarkSignedRpcRequest<'a> {
    p_share_id: &'a str,
    p_actor_user_id: &'a str,
    p_attachment_id: &'a str,
    p_upload_expires_at: &'a str,
    p_sha256: &'a str,
}

#[derive(Serialize)]
struct FinalizeRpcRequest<'a> {
    p_share_id: &'a str,
    p_actor_user_id: &'a str,
    p_attachment_id: &'a str,
    p_object_key: &'a str,
    p_observed_size_bytes: i64,
    p_observed_content_type: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedAttachmentRow {
    attachment_id: String,
    object_key: String,
    object_state: String,
    filename: String,
    content_type: String,
    size_bytes: i64,
    sha256: Option<String>,
    reservation_expires_at: String,
    upload_expires_at: Option<String>,
    cleanup_not_before: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReservedRow {
    attachment_id: String,
    object_key: String,
    object_state: String,
    filename: String,
    content_type: String,
    size_bytes: i64,
    sha256: Option<String>,
    reservation_expires_at: String,
    cleanup_not_before: String,
    was_created: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarkedSignedRow {
    attachment_id: String,
    object_key: String,
    object_state: String,
    filename: String,
    content_type: String,
    size_bytes: i64,
    sha256: String,
    upload_expires_at: String,
    cleanup_not_before: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FinalizedRow {
    attachment_id: String,
    object_key: String,
    object_state: String,
    was_finalized: bool,
}

#[derive(Deserialize)]
struct PostgrestError {
    code: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        reserve_shared_attachment,
        grant_shared_attachment_upload,
        finalize_shared_attachment
    ),
    components(schemas(
        ReserveSharedAttachmentRequest,
        ReservedSharedAttachment,
        SharedAttachmentObjectRequest,
        GrantSharedAttachmentUploadRequest,
        SharedAttachmentUploadGrant,
        FinalizedSharedAttachment
    ))
)]
struct SharedAttachmentsApiDoc;

pub(super) fn openapi() -> utoipa::openapi::OpenApi {
    SharedAttachmentsApiDoc::openapi()
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/shares/{share_id}/attachments/reserve",
            post(reserve_shared_attachment),
        )
        .route(
            "/shares/{share_id}/attachments/upload-grant",
            post(grant_shared_attachment_upload),
        )
        .route(
            "/shares/{share_id}/attachments/finalize",
            post(finalize_shared_attachment),
        )
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .layer(middleware::from_fn(add_no_store))
}

async fn add_no_store(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[utoipa::path(
    post,
    path = "/shares/{share_id}/attachments/reserve",
    tag = "sync",
    params(("share_id" = String, Path, description = "Session share ID")),
    request_body = ReserveSharedAttachmentRequest,
    responses(
        (status = 200, description = "Reserved immutable shared attachment", body = ReservedSharedAttachment),
        (status = 400, description = "Invalid attachment metadata"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Pro or share-manager access required"),
        (status = 409, description = "Attachment reservation conflict"),
        (status = 507, description = "Shared attachment quota exhausted"),
        (status = 502, description = "Shared attachment service unavailable")
    )
)]
async fn reserve_shared_attachment(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(share_id): Path<String>,
    Json(request): Json<ReserveSharedAttachmentRequest>,
) -> Result<Json<ReservedSharedAttachment>> {
    require_pro(&auth)?;
    let actor_user_id = canonical_uuid(&auth.claims.sub)?;
    let share_id = canonical_uuid(&share_id)?;
    validate_ref(&request.attachment_ref)?;
    validate_ref(&request.version_ref)?;
    validate_filename(&request.filename)?;
    validate_content_type(&request.content_type)?;
    if request.attachment_ref == request.version_ref
        || request.size_bytes == 0
        || request.size_bytes > MAX_SIZE_BYTES
    {
        return Err(invalid_request());
    }

    let row: ReservedRow = rpc_single(
        &state,
        "reserve_session_share_attachment",
        &ReserveRpcRequest {
            p_share_id: &share_id,
            p_actor_user_id: &actor_user_id,
            p_attachment_ref: &request.attachment_ref,
            p_version_ref: &request.version_ref,
            p_filename: &request.filename,
            p_content_type: &request.content_type,
            p_size_bytes: request.size_bytes as i64,
        },
    )
    .await?;
    validate_reserved(&row, &share_id, &request)?;

    Ok(Json(ReservedSharedAttachment {
        attachment_id: row.attachment_id,
        object_key: row.object_key,
        object_state: row.object_state,
        filename: row.filename,
        content_type: row.content_type,
        size_bytes: request.size_bytes,
        sha256: row.sha256,
        reservation_expires_at: row.reservation_expires_at,
        was_created: row.was_created,
    }))
}

#[utoipa::path(
    post,
    path = "/shares/{share_id}/attachments/upload-grant",
    tag = "sync",
    params(("share_id" = String, Path, description = "Session share ID")),
    request_body = GrantSharedAttachmentUploadRequest,
    responses(
        (status = 200, description = "Time-limited immutable upload grant", body = SharedAttachmentUploadGrant),
        (status = 400, description = "Invalid object key or checksum"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Pro or share-manager access required"),
        (status = 404, description = "Attachment reservation unavailable"),
        (status = 409, description = "Attachment state conflict"),
        (status = 502, description = "Shared attachment service unavailable")
    )
)]
async fn grant_shared_attachment_upload(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(share_id): Path<String>,
    Json(request): Json<GrantSharedAttachmentUploadRequest>,
) -> Result<Json<SharedAttachmentUploadGrant>> {
    require_pro(&auth)?;
    let actor_user_id = canonical_uuid(&auth.claims.sub)?;
    let share_id = canonical_uuid(&share_id)?;
    let object_key = validate_object_key(&request.object_key, &share_id)?;
    validate_sha256(&request.sha256)?;
    let object = read_object(&state, &share_id, &actor_user_id, &object_key).await?;
    if object
        .sha256
        .as_deref()
        .is_some_and(|hash| hash != request.sha256)
    {
        return Err(SyncError::SharedAttachmentConflict);
    }
    if object.object_state == "ready" {
        if object.sha256.as_deref() != Some(&request.sha256) {
            return Err(SyncError::SharedAttachmentConflict);
        }
        return Ok(Json(grant_response(object, request.sha256, None, None)?));
    }
    if object.object_state != "reserved" {
        return Err(SyncError::SharedAttachmentConflict);
    }

    let upload_expires_at = future_timestamp(UPLOAD_TTL_SECONDS)?;
    let marked: MarkedSignedRow = rpc_single(
        &state,
        "mark_session_share_attachment_signed",
        &MarkSignedRpcRequest {
            p_share_id: &share_id,
            p_actor_user_id: &actor_user_id,
            p_attachment_id: &object.attachment_id,
            p_upload_expires_at: &upload_expires_at,
            p_sha256: &request.sha256,
        },
    )
    .await?;
    validate_marked(&marked, &object, &request.sha256, &upload_expires_at)?;
    let signed = state
        .storage
        .create_signed_upload(BUCKET, &object_key)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "shared attachment upload signing failed");
            SyncError::SharedAttachmentServiceUnavailable
        })?;

    Ok(Json(SharedAttachmentUploadGrant {
        attachment_id: marked.attachment_id,
        object_key: marked.object_key,
        object_state: marked.object_state,
        filename: marked.filename,
        content_type: marked.content_type,
        size_bytes: valid_size(marked.size_bytes)?,
        sha256: marked.sha256,
        upload_expires_at: Some(marked.upload_expires_at),
        upload_token: Some(signed.token),
    }))
}

#[utoipa::path(
    post,
    path = "/shares/{share_id}/attachments/finalize",
    tag = "sync",
    params(("share_id" = String, Path, description = "Session share ID")),
    request_body = SharedAttachmentObjectRequest,
    responses(
        (status = 200, description = "Verified shared attachment", body = FinalizedSharedAttachment),
        (status = 400, description = "Invalid object key"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Pro or share-manager access required"),
        (status = 404, description = "Attachment unavailable"),
        (status = 409, description = "Uploaded object does not match the reservation"),
        (status = 502, description = "Shared attachment service unavailable"),
        (status = 503, description = "Shared attachment verification capacity is busy")
    )
)]
async fn finalize_shared_attachment(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(share_id): Path<String>,
    Json(request): Json<SharedAttachmentObjectRequest>,
) -> Result<Json<FinalizedSharedAttachment>> {
    require_pro(&auth)?;
    let actor_user_id = canonical_uuid(&auth.claims.sub)?;
    let share_id = canonical_uuid(&share_id)?;
    let object_key = validate_object_key(&request.object_key, &share_id)?;
    let object = read_object(&state, &share_id, &actor_user_id, &object_key).await?;
    if object.object_state == "ready" {
        return Ok(Json(FinalizedSharedAttachment {
            attachment_id: object.attachment_id,
            object_key,
            object_state: object.object_state,
            was_finalized: false,
        }));
    }
    if object.object_state != "reserved" {
        return Err(SyncError::SharedAttachmentConflict);
    }
    let cleanup_not_before = parse_timestamp(&object.cleanup_not_before)
        .ok_or(SyncError::SharedAttachmentServiceUnavailable)?;
    if cleanup_not_before.with_timezone(&Utc) <= Utc::now() {
        return Err(SyncError::SharedAttachmentConflict);
    }
    let expected_sha256 = object
        .sha256
        .as_deref()
        .ok_or(SyncError::SharedAttachmentConflict)?;
    let metadata = state
        .storage
        .object_metadata(BUCKET, &object_key)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "shared attachment object verification failed");
            SyncError::SharedAttachmentServiceUnavailable
        })?;
    let observed_sha256 = metadata
        .user_metadata
        .get("plaintextSha256")
        .and_then(Value::as_str);
    let expected_size = valid_size(object.size_bytes)?;
    if metadata.size_bytes != expected_size
        || metadata.content_type != object.content_type
        || observed_sha256 != Some(expected_sha256)
    {
        return Err(SyncError::SharedAttachmentConflict);
    }
    let _verification_slot = state
        .attachment_verification_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| SyncError::SharedAttachmentVerificationBusy)?;
    let observed_sha256 = state
        .storage
        .object_sha256(BUCKET, &object_key, expected_size)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "shared attachment object checksum verification failed");
            SyncError::SharedAttachmentServiceUnavailable
        })?;
    if observed_sha256 != expected_sha256 {
        return Err(SyncError::SharedAttachmentConflict);
    }

    let row: FinalizedRow = rpc_single(
        &state,
        "finalize_session_share_attachment",
        &FinalizeRpcRequest {
            p_share_id: &share_id,
            p_actor_user_id: &actor_user_id,
            p_attachment_id: &object.attachment_id,
            p_object_key: &object_key,
            p_observed_size_bytes: object.size_bytes,
            p_observed_content_type: &object.content_type,
        },
    )
    .await?;
    if canonical_uuid_v4(&row.attachment_id).as_deref() != Some(object.attachment_id.as_str())
        || row.object_key != object_key
        || row.object_state != "ready"
    {
        return Err(SyncError::SharedAttachmentServiceUnavailable);
    }

    Ok(Json(FinalizedSharedAttachment {
        attachment_id: row.attachment_id,
        object_key: row.object_key,
        object_state: row.object_state,
        was_finalized: row.was_finalized,
    }))
}

async fn read_object(
    state: &AppState,
    share_id: &str,
    actor_user_id: &str,
    object_key: &str,
) -> Result<SharedAttachmentRow> {
    let row: SharedAttachmentRow = rpc_single(
        state,
        "read_session_share_attachment_by_key",
        &ObjectKeyRpcRequest {
            p_share_id: share_id,
            p_actor_user_id: actor_user_id,
            p_object_key: object_key,
        },
    )
    .await?;
    validate_object_row(&row, share_id, object_key)?;
    Ok(row)
}

async fn rpc_single<RequestBody, Row>(
    state: &AppState,
    function: &str,
    request: &RequestBody,
) -> Result<Row>
where
    RequestBody: Serialize + ?Sized,
    Row: DeserializeOwned,
{
    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/{function}",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(RPC_TIMEOUT)
        .json(request)
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(function, %error, "shared attachment ledger request failed");
            SyncError::SharedAttachmentServiceUnavailable
        })?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|_| SyncError::SharedAttachmentServiceUnavailable)?;
    if bytes.len() > MAX_RPC_RESPONSE_BYTES {
        return Err(SyncError::SharedAttachmentServiceUnavailable);
    }
    if !status.is_success() {
        let code = serde_json::from_slice::<PostgrestError>(&bytes)
            .ok()
            .map(|error| error.code);
        return Err(match (status, code.as_deref()) {
            (StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN, _) | (_, Some("42501")) => {
                SyncError::SharedAttachmentForbidden
            }
            (_, Some("22023")) => invalid_request(),
            (_, Some("40001" | "55000")) => SyncError::SharedAttachmentConflict,
            (_, Some("54000")) => SyncError::SharedAttachmentQuotaExceeded,
            _ => SyncError::SharedAttachmentServiceUnavailable,
        });
    }
    let mut rows = serde_json::from_slice::<Vec<Row>>(&bytes)
        .map_err(|_| SyncError::SharedAttachmentServiceUnavailable)?;
    match rows.len() {
        0 => Err(SyncError::SharedAttachmentNotFound),
        1 => Ok(rows.pop().expect("row count was checked")),
        _ => Err(SyncError::SharedAttachmentServiceUnavailable),
    }
}

fn grant_response(
    object: SharedAttachmentRow,
    sha256: String,
    upload_expires_at: Option<String>,
    upload_token: Option<String>,
) -> Result<SharedAttachmentUploadGrant> {
    Ok(SharedAttachmentUploadGrant {
        attachment_id: object.attachment_id,
        object_key: object.object_key,
        object_state: object.object_state,
        filename: object.filename,
        content_type: object.content_type,
        size_bytes: valid_size(object.size_bytes)?,
        sha256,
        upload_expires_at,
        upload_token,
    })
}

fn validate_reserved(
    row: &ReservedRow,
    share_id: &str,
    request: &ReserveSharedAttachmentRequest,
) -> Result<()> {
    canonical_uuid_v4(&row.attachment_id).ok_or(SyncError::SharedAttachmentServiceUnavailable)?;
    validate_object_key(&row.object_key, share_id)
        .map_err(|_| SyncError::SharedAttachmentServiceUnavailable)?;
    if !matches!(row.object_state.as_str(), "reserved" | "ready")
        || row.filename != request.filename
        || row.content_type != request.content_type
        || row.size_bytes != request.size_bytes as i64
        || row
            .sha256
            .as_deref()
            .is_some_and(|value| validate_sha256(value).is_err())
        || parse_timestamp(&row.reservation_expires_at).is_none()
        || parse_timestamp(&row.cleanup_not_before).is_none()
    {
        return Err(SyncError::SharedAttachmentServiceUnavailable);
    }
    Ok(())
}

fn validate_object_row(row: &SharedAttachmentRow, share_id: &str, object_key: &str) -> Result<()> {
    canonical_uuid_v4(&row.attachment_id).ok_or(SyncError::SharedAttachmentServiceUnavailable)?;
    if row.object_key != object_key
        || validate_object_key(&row.object_key, share_id).is_err()
        || !matches!(row.object_state.as_str(), "reserved" | "ready")
        || validate_filename(&row.filename).is_err()
        || validate_content_type(&row.content_type).is_err()
        || valid_size(row.size_bytes).is_err()
        || row
            .sha256
            .as_deref()
            .is_some_and(|value| validate_sha256(value).is_err())
        || parse_timestamp(&row.reservation_expires_at).is_none()
        || row
            .upload_expires_at
            .as_deref()
            .is_some_and(|value| parse_timestamp(value).is_none())
        || parse_timestamp(&row.cleanup_not_before).is_none()
    {
        return Err(SyncError::SharedAttachmentServiceUnavailable);
    }
    Ok(())
}

fn validate_marked(
    row: &MarkedSignedRow,
    object: &SharedAttachmentRow,
    sha256: &str,
    expires_at: &str,
) -> Result<()> {
    if row.attachment_id != object.attachment_id
        || row.object_key != object.object_key
        || row.object_state != "reserved"
        || row.filename != object.filename
        || row.content_type != object.content_type
        || row.size_bytes != object.size_bytes
        || row.sha256 != sha256
        || row.upload_expires_at != expires_at
        || parse_timestamp(&row.cleanup_not_before).is_none()
    {
        return Err(SyncError::SharedAttachmentServiceUnavailable);
    }
    Ok(())
}

fn require_pro(auth: &AuthContext) -> Result<()> {
    auth.claims
        .is_pro()
        .then_some(())
        .ok_or(SyncError::ProPlanRequired)
}

fn canonical_uuid(value: &str) -> Result<String> {
    let uuid = Uuid::parse_str(value).map_err(|_| invalid_request())?;
    let canonical = uuid.to_string();
    (canonical == value)
        .then_some(canonical)
        .ok_or_else(invalid_request)
}

fn canonical_uuid_v4(value: &str) -> Option<String> {
    let uuid = Uuid::parse_str(value).ok()?;
    let canonical = uuid.to_string();
    (canonical == value && uuid.get_version() == Some(Version::Random)).then_some(canonical)
}

fn validate_object_key(value: &str, share_id: &str) -> Result<String> {
    let mut parts = value.split('/');
    let owner = parts.next().and_then(|owner| canonical_uuid(owner).ok());
    let share = parts.next();
    let attachment_id = parts
        .next()
        .and_then(|filename| filename.strip_suffix(".sna1"))
        .and_then(canonical_uuid_v4);
    (parts.next().is_none()
        && owner.is_some()
        && share == Some(share_id)
        && attachment_id.is_some())
    .then(|| value.to_string())
    .ok_or_else(invalid_request)
}

fn validate_ref(value: &str) -> Result<()> {
    (value.len() == 43
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
    .then_some(())
    .ok_or_else(invalid_request)
}

fn validate_filename(value: &str) -> Result<()> {
    (!value.is_empty()
        && value.len() <= 1024
        && value.trim() == value
        && !value.contains(['/', '\\'])
        && !value.chars().any(char::is_control))
    .then_some(())
    .ok_or_else(invalid_request)
}

fn validate_content_type(value: &str) -> Result<()> {
    let forbidden = matches!(
        value,
        "text/html"
            | "image/svg+xml"
            | "application/xhtml+xml"
            | "application/xml"
            | "text/xml"
            | "application/javascript"
            | "text/javascript"
    );
    let valid = value.len() <= 255
        && value == value.to_ascii_lowercase()
        && value.split_once('/').is_some_and(|(kind, subtype)| {
            !kind.is_empty()
                && !subtype.is_empty()
                && kind.bytes().chain(subtype.bytes()).all(|byte| {
                    byte.is_ascii_alphanumeric()
                        || matches!(
                            byte,
                            b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-'
                        )
                })
        });
    (valid && !forbidden)
        .then_some(())
        .ok_or_else(invalid_request)
}

fn validate_sha256(value: &str) -> Result<()> {
    (value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')))
    .then_some(())
    .ok_or_else(invalid_request)
}

fn valid_size(value: i64) -> Result<u64> {
    u64::try_from(value)
        .ok()
        .filter(|value| (1..=MAX_SIZE_BYTES).contains(value))
        .ok_or(SyncError::SharedAttachmentServiceUnavailable)
}

fn future_timestamp(seconds: i64) -> Result<String> {
    Utc::now()
        .checked_add_signed(TimeDelta::seconds(seconds))
        .map(|value| value.to_rfc3339_opts(SecondsFormat::Millis, true))
        .ok_or_else(|| SyncError::Internal("shared attachment expiry overflow".to_string()))
}

fn parse_timestamp(value: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    chrono::DateTime::parse_from_rfc3339(value).ok()
}

fn invalid_request() -> SyncError {
    SyncError::BadRequest("Shared attachment request is invalid".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_workspace_owner_object_keys_for_other_managers() {
        let share_id = "11111111-1111-4111-8111-111111111111";
        let owner_id = "22222222-2222-4222-8222-222222222222";
        let attachment_id = "33333333-3333-4333-8333-333333333333";
        let object_key = format!("{owner_id}/{share_id}/{attachment_id}.sna1");

        assert_eq!(
            validate_object_key(&object_key, share_id).unwrap(),
            object_key
        );
    }

    #[test]
    fn rejects_cross_share_and_active_content_object_metadata() {
        assert!(
            validate_object_key(
                "22222222-2222-4222-8222-222222222222/11111111-1111-4111-8111-111111111111/33333333-3333-4333-8333-333333333333.sna1",
                "44444444-4444-4444-8444-444444444444",
            )
            .is_err()
        );
        assert!(validate_content_type("text/html").is_err());
        assert!(validate_content_type("image/svg+xml").is_err());
        assert!(validate_content_type("image/png").is_ok());
    }
}

use std::time::Duration;

use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{HeaderValue, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post, put},
};
use chrono::{SecondsFormat, TimeDelta, Utc};
use hypr_api_auth::AuthContext;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use utoipa::OpenApi;
use uuid::{Uuid, Version};

use crate::{
    error::{Result, SyncError},
    state::AppState,
};

const ATTACHMENT_BACKUP_BUCKET: &str = "attachment-backups";
const BACKUP_RPC_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_BACKUP_REQUEST_BYTES: usize = 4 * 1024;
const MAX_BACKUP_RPC_RESPONSE_BYTES: usize = 16 * 1024;
const MAX_CIPHERTEXT_SIZE_BYTES: u64 = 545_259_520;
const FORMAT_VERSION: i16 = 1;
const UPLOAD_TOKEN_TTL_SECONDS: i64 = 2 * 60 * 60;
const UPLOAD_CLEANUP_GRACE_SECONDS: i64 = 24 * 60 * 60 + 5 * 60;
const DOWNLOAD_URL_TTL_SECONDS: i64 = 15 * 60;
const DOWNLOAD_CLEANUP_GRACE_SECONDS: i64 = 5 * 60;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ReserveAttachmentBackupRequest {
    attachment_ref: String,
    version_ref: String,
    ciphertext_size_bytes: u64,
    format_version: i16,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReservedAttachmentBackup {
    object_id: String,
    object_key: String,
    object_state: String,
    ciphertext_size_bytes: u64,
    format_version: i16,
    reservation_expires_at: String,
    ciphertext_sha256: Option<String>,
    was_created: bool,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AttachmentBackupObjectRequest {
    object_key: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct GrantAttachmentBackupUploadRequest {
    object_key: String,
    ciphertext_sha256: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AttachmentBackupUploadGrant {
    object_id: String,
    object_key: String,
    object_state: String,
    ciphertext_size_bytes: u64,
    ciphertext_sha256: String,
    format_version: i16,
    upload_expires_at: Option<String>,
    upload_token: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FinalizedAttachmentBackup {
    object_key: String,
    object_state: String,
    was_finalized: bool,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PromoteAttachmentBackupRequest {
    object_key: String,
    expected_current_object_key: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct PromotedAttachmentBackup {
    current_object_key: String,
    current_version_ref: String,
    current_ciphertext_sha256: String,
    displaced_object_key: Option<String>,
    was_promoted: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentAttachmentBackup {
    version_ref: String,
    object_key: String,
    ciphertext_sha256: String,
    ciphertext_size_bytes: u64,
    format_version: i16,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AttachmentBackupDownload {
    object_id: String,
    object_key: String,
    ciphertext_size_bytes: u64,
    ciphertext_sha256: String,
    format_version: i16,
    signed_url: String,
    expires_at: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DeleteAttachmentBackupRequest {
    object_key: String,
    attachment_ref: String,
    version_ref: String,
    delete_request_id: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScheduledAttachmentBackupDeletion {
    object_key: String,
    attachment_ref: String,
    version_ref: String,
    delete_request_id: String,
    delete_fence_id: String,
    delete_generation: i64,
    delete_not_before: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CanceledAttachmentBackupDeletion {
    object_key: String,
    attachment_ref: String,
    version_ref: String,
    delete_request_id: String,
}

#[derive(Serialize)]
struct ReserveRpcRequest<'a> {
    p_owner_user_id: &'a str,
    p_attachment_ref: &'a str,
    p_version_ref: &'a str,
    p_ciphertext_size_bytes: i64,
    p_format_version: i16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReservedRow {
    object_id: String,
    object_key: String,
    object_state: String,
    ciphertext_size_bytes: i64,
    format_version: i16,
    reservation_expires_at: String,
    cleanup_not_before: String,
    ciphertext_sha256: Option<String>,
    was_created: bool,
}

#[derive(Serialize)]
struct MarkSignedRpcRequest<'a> {
    p_owner_user_id: &'a str,
    p_object_id: &'a str,
    p_upload_expires_at: &'a str,
    p_ciphertext_sha256: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarkedSignedRow {
    object_id: String,
    object_key: String,
    last_signed_at: String,
    upload_expires_at: String,
    cleanup_not_before: String,
    ciphertext_sha256: String,
}

#[derive(Serialize)]
struct ObjectKeyRpcRequest<'a> {
    p_owner_user_id: &'a str,
    p_object_key: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupObjectRow {
    object_id: String,
    attachment_ref: String,
    version_ref: String,
    object_key: String,
    object_state: String,
    ciphertext_size_bytes: i64,
    format_version: i16,
    reservation_expires_at: String,
    upload_expires_at: Option<String>,
    cleanup_not_before: String,
    ciphertext_sha256: Option<String>,
}

#[derive(Serialize)]
struct FinalizeRpcRequest<'a> {
    p_owner_user_id: &'a str,
    p_object_id: &'a str,
    p_object_key: &'a str,
    p_observed_ciphertext_size_bytes: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FinalizedRow {
    object_id: String,
    object_key: String,
    object_state: String,
    was_finalized: bool,
}

#[derive(Serialize)]
struct PromoteRpcRequest<'a> {
    p_owner_user_id: &'a str,
    p_candidate_object_id: &'a str,
    p_candidate_object_key: &'a str,
    p_expected_current_object_key: Option<&'a str>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PromotedRow {
    current_object_id: String,
    current_object_key: String,
    current_version_ref: String,
    current_ciphertext_sha256: String,
    displaced_object_id: Option<String>,
    displaced_object_key: Option<String>,
    was_promoted: bool,
}

#[derive(Serialize)]
struct CurrentRpcRequest<'a> {
    p_owner_user_id: &'a str,
    p_attachment_ref: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentRow {
    object_id: String,
    version_ref: String,
    object_key: String,
    ciphertext_sha256: String,
    ciphertext_size_bytes: i64,
    format_version: i16,
}

#[derive(Serialize)]
struct PrepareDownloadRpcRequest<'a> {
    p_owner_user_id: &'a str,
    p_object_key: &'a str,
    p_download_expires_at: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparedDownloadRow {
    object_id: String,
    object_key: String,
    ciphertext_sha256: String,
    ciphertext_size_bytes: i64,
    format_version: i16,
    cleanup_not_before: String,
}

#[derive(Serialize)]
struct DeleteRpcRequest<'a> {
    p_owner_user_id: &'a str,
    p_attachment_ref: &'a str,
    p_version_ref: &'a str,
    p_object_key: &'a str,
    p_delete_request_id: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduledDeletionRow {
    outcome: String,
    object_id: Option<String>,
    object_key: String,
    delete_request_id: String,
    delete_fence_id: Option<String>,
    delete_generation: Option<i64>,
    delete_not_before: Option<String>,
    #[serde(rename = "was_created")]
    _was_created: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CanceledDeletionRow {
    outcome: String,
    object_key: String,
    #[serde(rename = "was_cancelled")]
    _was_cancelled: bool,
}

#[derive(Deserialize)]
struct PostgrestError {
    code: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        reserve_attachment_backup,
        grant_attachment_backup_upload,
        finalize_attachment_backup,
        promote_attachment_backup,
        read_current_attachment_backup,
        download_attachment_backup,
        delete_attachment_backup,
        cancel_attachment_backup_deletion
    ),
    components(schemas(
        ReserveAttachmentBackupRequest,
        ReservedAttachmentBackup,
        AttachmentBackupObjectRequest,
        GrantAttachmentBackupUploadRequest,
        AttachmentBackupUploadGrant,
        FinalizedAttachmentBackup,
        PromoteAttachmentBackupRequest,
        PromotedAttachmentBackup,
        CurrentAttachmentBackup,
        AttachmentBackupDownload,
        DeleteAttachmentBackupRequest,
        ScheduledAttachmentBackupDeletion,
        CanceledAttachmentBackupDeletion
    ))
)]
struct AttachmentBackupsApiDoc;

pub(super) fn openapi() -> utoipa::openapi::OpenApi {
    AttachmentBackupsApiDoc::openapi()
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/attachment-backups/reserve",
            post(reserve_attachment_backup),
        )
        .route(
            "/attachment-backups/upload-grant",
            post(grant_attachment_backup_upload),
        )
        .route(
            "/attachment-backups/finalize",
            post(finalize_attachment_backup),
        )
        .route("/attachment-backups/head", put(promote_attachment_backup))
        .route(
            "/attachment-backups/head/{attachment_ref}",
            get(read_current_attachment_backup),
        )
        .route(
            "/attachment-backups/download",
            post(download_attachment_backup),
        )
        .route("/attachment-backups/delete", post(delete_attachment_backup))
        .route(
            "/attachment-backups/delete/cancel",
            post(cancel_attachment_backup_deletion),
        )
        .layer(DefaultBodyLimit::max(MAX_BACKUP_REQUEST_BYTES))
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
    path = "/attachment-backups/reserve",
    tag = "sync",
    request_body = ReserveAttachmentBackupRequest,
    responses(
        (status = 200, description = "Reserved immutable backup identity", body = ReservedAttachmentBackup),
        (status = 400, description = "Invalid backup metadata"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription or backup access required"),
        (status = 409, description = "Backup reservation conflict"),
        (status = 507, description = "Backup quota exhausted"),
        (status = 502, description = "Backup service unavailable")
    )
)]
async fn reserve_attachment_backup(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(request): Json<ReserveAttachmentBackupRequest>,
) -> Result<Json<ReservedAttachmentBackup>> {
    require_pro(&auth)?;
    let owner_user_id = canonical_owner(&auth)?;
    validate_ref(&request.attachment_ref, "attachment")?;
    validate_ref(&request.version_ref, "version")?;
    if request.attachment_ref == request.version_ref
        || request.ciphertext_size_bytes == 0
        || request.ciphertext_size_bytes > MAX_CIPHERTEXT_SIZE_BYTES
        || request.format_version != FORMAT_VERSION
    {
        return Err(invalid_request());
    }

    let row: ReservedRow = rpc_single(
        &state,
        "reserve_attachment_backup",
        &ReserveRpcRequest {
            p_owner_user_id: &owner_user_id,
            p_attachment_ref: &request.attachment_ref,
            p_version_ref: &request.version_ref,
            p_ciphertext_size_bytes: request.ciphertext_size_bytes as i64,
            p_format_version: request.format_version,
        },
    )
    .await
    .map_err(map_reservation_error)?;
    validate_reserved_row(&row, &owner_user_id, &request)?;

    Ok(Json(ReservedAttachmentBackup {
        object_id: row.object_id,
        object_key: row.object_key,
        object_state: row.object_state,
        ciphertext_size_bytes: request.ciphertext_size_bytes,
        format_version: row.format_version,
        reservation_expires_at: row.reservation_expires_at,
        ciphertext_sha256: row.ciphertext_sha256,
        was_created: row.was_created,
    }))
}

#[utoipa::path(
    post,
    path = "/attachment-backups/upload-grant",
    tag = "sync",
    request_body = GrantAttachmentBackupUploadRequest,
    responses(
        (status = 200, description = "Time-limited grant for an immutable backup upload", body = AttachmentBackupUploadGrant),
        (status = 400, description = "Invalid object key or ciphertext hash"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription or backup access required"),
        (status = 404, description = "Backup reservation unavailable"),
        (status = 409, description = "Backup state or ciphertext hash conflict"),
        (status = 502, description = "Backup service unavailable")
    )
)]
async fn grant_attachment_backup_upload(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(request): Json<GrantAttachmentBackupUploadRequest>,
) -> Result<Json<AttachmentBackupUploadGrant>> {
    require_pro(&auth)?;
    let owner_user_id = canonical_owner(&auth)?;
    let object_key = validate_object_key(&request.object_key, &owner_user_id)?;
    validate_sha256(&request.ciphertext_sha256)?;
    let object = read_backup_object(&state, &owner_user_id, &object_key).await?;
    if object
        .ciphertext_sha256
        .as_deref()
        .is_some_and(|hash| hash != request.ciphertext_sha256)
    {
        return Err(SyncError::AttachmentBackupConflict);
    }
    if object.object_state != "reserved" {
        if !matches!(object.object_state.as_str(), "ready" | "current")
            || object.ciphertext_sha256.as_deref() != Some(&request.ciphertext_sha256)
        {
            return Err(SyncError::AttachmentBackupConflict);
        }
        return Ok(Json(AttachmentBackupUploadGrant {
            object_id: object.object_id,
            object_key,
            object_state: object.object_state,
            ciphertext_size_bytes: valid_size(object.ciphertext_size_bytes)?,
            ciphertext_sha256: request.ciphertext_sha256,
            format_version: object.format_version,
            upload_expires_at: None,
            upload_token: None,
        }));
    }

    let upload_expires_at = future_timestamp(UPLOAD_TOKEN_TTL_SECONDS)?;
    let marked: MarkedSignedRow = rpc_single(
        &state,
        "mark_attachment_backup_signed",
        &MarkSignedRpcRequest {
            p_owner_user_id: &owner_user_id,
            p_object_id: &object.object_id,
            p_upload_expires_at: &upload_expires_at,
            p_ciphertext_sha256: &request.ciphertext_sha256,
        },
    )
    .await
    .map_err(map_backup_error)?;
    validate_marked_signed_row(
        &marked,
        &object,
        &request.ciphertext_sha256,
        &upload_expires_at,
    )?;

    let signed_upload = state
        .storage
        .create_signed_upload(ATTACHMENT_BACKUP_BUCKET, &object_key)
        .await
        .map_err(|_| {
            tracing::warn!("Supabase Storage upload signing failed");
            SyncError::AttachmentBackupServiceUnavailable
        })?;

    Ok(Json(AttachmentBackupUploadGrant {
        object_id: object.object_id,
        object_key,
        object_state: object.object_state,
        ciphertext_size_bytes: valid_size(object.ciphertext_size_bytes)?,
        ciphertext_sha256: marked.ciphertext_sha256,
        format_version: object.format_version,
        upload_expires_at: Some(marked.upload_expires_at),
        upload_token: Some(signed_upload.token),
    }))
}

#[utoipa::path(
    post,
    path = "/attachment-backups/finalize",
    tag = "sync",
    request_body = AttachmentBackupObjectRequest,
    responses(
        (status = 200, description = "Uploaded backup verified and finalized", body = FinalizedAttachmentBackup),
        (status = 400, description = "Invalid object key"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription or backup access required"),
        (status = 404, description = "Backup unavailable"),
        (status = 409, description = "Uploaded object does not match its reservation"),
        (status = 502, description = "Backup service unavailable"),
        (status = 503, description = "Backup verification capacity is busy")
    )
)]
async fn finalize_attachment_backup(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(request): Json<AttachmentBackupObjectRequest>,
) -> Result<Json<FinalizedAttachmentBackup>> {
    require_pro(&auth)?;
    let owner_user_id = canonical_owner(&auth)?;
    let object_key = validate_object_key(&request.object_key, &owner_user_id)?;
    let object = read_backup_object(&state, &owner_user_id, &object_key).await?;
    if matches!(object.object_state.as_str(), "ready" | "current") {
        return Ok(Json(FinalizedAttachmentBackup {
            object_key,
            object_state: object.object_state,
            was_finalized: false,
        }));
    }
    if object.object_state != "reserved" {
        return Err(SyncError::AttachmentBackupConflict);
    }
    let cleanup_not_before = parse_timestamp(&object.cleanup_not_before)
        .ok_or_else(|| invalid_upstream_response("object lookup"))?;
    if cleanup_not_before <= Utc::now() {
        return Err(SyncError::AttachmentBackupConflict);
    }
    let expected_ciphertext_sha256 = object
        .ciphertext_sha256
        .as_deref()
        .ok_or(SyncError::AttachmentBackupConflict)?;

    let object_info = state
        .storage
        .object_info(ATTACHMENT_BACKUP_BUCKET, &object_key)
        .await
        .map_err(|_| {
            tracing::warn!("Supabase Storage object verification failed");
            SyncError::AttachmentBackupServiceUnavailable
        })?;
    let expected_size = valid_size(object.ciphertext_size_bytes)?;
    if object_info.size_bytes != expected_size
        || object_info.content_type != "application/octet-stream"
        || object_info.ciphertext_sha256 != expected_ciphertext_sha256
        || i16::from(object_info.format_version) != object.format_version
    {
        tracing::warn!(
            expected_size,
            observed_size = object_info.size_bytes,
            "Attachment backup object metadata did not match the reservation"
        );
        return Err(SyncError::AttachmentBackupConflict);
    }
    let _verification_slot = state
        .attachment_verification_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| SyncError::AttachmentBackupVerificationBusy)?;
    let observed_ciphertext_sha256 = state
        .storage
        .object_sha256(ATTACHMENT_BACKUP_BUCKET, &object_key, expected_size)
        .await
        .map_err(|_| {
            tracing::warn!("Supabase Storage object checksum verification failed");
            SyncError::AttachmentBackupServiceUnavailable
        })?;
    if observed_ciphertext_sha256 != expected_ciphertext_sha256 {
        tracing::warn!("Attachment backup object checksum did not match the reservation");
        return Err(SyncError::AttachmentBackupConflict);
    }

    let row: FinalizedRow = rpc_single(
        &state,
        "finalize_attachment_backup",
        &FinalizeRpcRequest {
            p_owner_user_id: &owner_user_id,
            p_object_id: &object.object_id,
            p_object_key: &object_key,
            p_observed_ciphertext_size_bytes: object.ciphertext_size_bytes,
        },
    )
    .await
    .map_err(map_backup_error)?;
    if canonical_object_id(&row.object_id).as_deref() != Some(object.object_id.as_str())
        || row.object_key != object_key
        || !matches!(row.object_state.as_str(), "ready" | "current")
    {
        return Err(invalid_upstream_response("finalization"));
    }

    Ok(Json(FinalizedAttachmentBackup {
        object_key: row.object_key,
        object_state: row.object_state,
        was_finalized: row.was_finalized,
    }))
}

#[utoipa::path(
    put,
    path = "/attachment-backups/head",
    tag = "sync",
    request_body = PromoteAttachmentBackupRequest,
    responses(
        (status = 200, description = "Backup promoted with compare-and-swap semantics", body = PromotedAttachmentBackup),
        (status = 400, description = "Invalid object key"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription or backup access required"),
        (status = 404, description = "Backup unavailable"),
        (status = 409, description = "Current backup changed"),
        (status = 502, description = "Backup service unavailable")
    )
)]
async fn promote_attachment_backup(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(request): Json<PromoteAttachmentBackupRequest>,
) -> Result<Json<PromotedAttachmentBackup>> {
    require_pro(&auth)?;
    let owner_user_id = canonical_owner(&auth)?;
    let object_key = validate_object_key(&request.object_key, &owner_user_id)?;
    let expected_current_object_key = request
        .expected_current_object_key
        .as_deref()
        .map(|key| validate_object_key(key, &owner_user_id))
        .transpose()?;
    let candidate = read_backup_object(&state, &owner_user_id, &object_key).await?;
    if !matches!(candidate.object_state.as_str(), "ready" | "current") {
        return Err(SyncError::AttachmentBackupConflict);
    }

    let row: PromotedRow = rpc_single(
        &state,
        "promote_attachment_backup",
        &PromoteRpcRequest {
            p_owner_user_id: &owner_user_id,
            p_candidate_object_id: &candidate.object_id,
            p_candidate_object_key: &object_key,
            p_expected_current_object_key: expected_current_object_key.as_deref(),
        },
    )
    .await
    .map_err(map_backup_error)?;
    validate_promoted_row(
        &row,
        &owner_user_id,
        &candidate,
        expected_current_object_key.as_deref(),
    )?;

    Ok(Json(PromotedAttachmentBackup {
        current_object_key: row.current_object_key,
        current_version_ref: row.current_version_ref,
        current_ciphertext_sha256: row.current_ciphertext_sha256,
        displaced_object_key: row.displaced_object_key,
        was_promoted: row.was_promoted,
    }))
}

#[utoipa::path(
    get,
    path = "/attachment-backups/head/{attachment_ref}",
    tag = "sync",
    params(("attachment_ref" = String, Path, description = "Blind attachment reference")),
    responses(
        (status = 200, description = "Current backup head", body = CurrentAttachmentBackup),
        (status = 400, description = "Invalid attachment reference"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription or backup access required"),
        (status = 404, description = "Current backup unavailable"),
        (status = 502, description = "Backup service unavailable")
    )
)]
async fn read_current_attachment_backup(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(attachment_ref): Path<String>,
) -> Result<Json<CurrentAttachmentBackup>> {
    require_pro(&auth)?;
    let owner_user_id = canonical_owner(&auth)?;
    validate_ref(&attachment_ref, "attachment")?;
    let row: CurrentRow = rpc_single(
        &state,
        "read_current_attachment_backup",
        &CurrentRpcRequest {
            p_owner_user_id: &owner_user_id,
            p_attachment_ref: &attachment_ref,
        },
    )
    .await
    .map_err(map_backup_error)?;
    let object_key = validate_object_key(&row.object_key, &owner_user_id)
        .map_err(|_| invalid_upstream_response("current backup"))?;
    canonical_object_id(&row.object_id)
        .ok_or_else(|| invalid_upstream_response("current backup"))?;
    let ciphertext_size_bytes = valid_size(row.ciphertext_size_bytes)?;
    if validate_ref(&row.version_ref, "version").is_err()
        || validate_sha256(&row.ciphertext_sha256).is_err()
        || row.format_version != FORMAT_VERSION
    {
        return Err(invalid_upstream_response("current backup"));
    }

    Ok(Json(CurrentAttachmentBackup {
        version_ref: row.version_ref,
        object_key,
        ciphertext_sha256: row.ciphertext_sha256,
        ciphertext_size_bytes,
        format_version: row.format_version,
    }))
}

#[utoipa::path(
    post,
    path = "/attachment-backups/download",
    tag = "sync",
    request_body = AttachmentBackupObjectRequest,
    responses(
        (status = 200, description = "Short-lived download for the server-current backup", body = AttachmentBackupDownload),
        (status = 400, description = "Invalid object key"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription or backup access required"),
        (status = 404, description = "Current backup unavailable"),
        (status = 409, description = "Backup is no longer current"),
        (status = 502, description = "Backup service unavailable")
    )
)]
async fn download_attachment_backup(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(request): Json<AttachmentBackupObjectRequest>,
) -> Result<Json<AttachmentBackupDownload>> {
    require_pro(&auth)?;
    let owner_user_id = canonical_owner(&auth)?;
    let object_key = validate_object_key(&request.object_key, &owner_user_id)?;
    let expires_at = future_timestamp(DOWNLOAD_URL_TTL_SECONDS)?;
    let row: PreparedDownloadRow = rpc_single(
        &state,
        "prepare_attachment_backup_download",
        &PrepareDownloadRpcRequest {
            p_owner_user_id: &owner_user_id,
            p_object_key: &object_key,
            p_download_expires_at: &expires_at,
        },
    )
    .await
    .map_err(map_backup_error)?;
    validate_prepared_download(&row, &owner_user_id, &object_key, &expires_at)?;

    let signed_url = state
        .storage
        .create_signed_url(
            ATTACHMENT_BACKUP_BUCKET,
            &object_key,
            DOWNLOAD_URL_TTL_SECONDS as u64,
        )
        .await
        .map_err(|_| {
            tracing::warn!("Supabase Storage download signing failed");
            SyncError::AttachmentBackupServiceUnavailable
        })?;

    Ok(Json(AttachmentBackupDownload {
        object_id: row.object_id,
        object_key,
        ciphertext_size_bytes: valid_size(row.ciphertext_size_bytes)?,
        ciphertext_sha256: row.ciphertext_sha256,
        format_version: row.format_version,
        signed_url,
        expires_at,
    }))
}

#[utoipa::path(
    post,
    path = "/attachment-backups/delete",
    tag = "sync",
    request_body = DeleteAttachmentBackupRequest,
    responses(
        (status = 200, description = "Backup deletion scheduled behind a dependency fence", body = ScheduledAttachmentBackupDeletion),
        (status = 400, description = "Invalid deletion identity"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription or backup access required"),
        (status = 404, description = "Backup unavailable"),
        (status = 409, description = "Backup changed, deletion was canceled, or a dependency appeared"),
        (status = 502, description = "Backup service unavailable")
    )
)]
async fn delete_attachment_backup(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(request): Json<DeleteAttachmentBackupRequest>,
) -> Result<Json<ScheduledAttachmentBackupDeletion>> {
    require_pro(&auth)?;
    let owner_user_id = canonical_owner(&auth)?;
    let request = validate_delete_request(request, &owner_user_id)?;
    let row: ScheduledDeletionRow = rpc_single(
        &state,
        "schedule_attachment_backup_deletion",
        &DeleteRpcRequest {
            p_owner_user_id: &owner_user_id,
            p_attachment_ref: &request.attachment_ref,
            p_version_ref: &request.version_ref,
            p_object_key: &request.object_key,
            p_delete_request_id: &request.delete_request_id,
        },
    )
    .await
    .map_err(map_deletion_error)?;

    validate_deletion_row_identity(
        &row.object_key,
        &row.delete_request_id,
        &request,
        "deletion scheduling",
    )?;
    match row.outcome.as_str() {
        "scheduled" => {}
        "dependency_appeared" => return Err(SyncError::AttachmentBackupDependencyAppeared),
        "cancelled" => return Err(SyncError::AttachmentBackupDeleteCancelled),
        _ => return Err(invalid_upstream_response("deletion scheduling")),
    }

    row.object_id
        .as_deref()
        .and_then(canonical_object_id)
        .ok_or_else(|| invalid_upstream_response("deletion scheduling"))?;
    let delete_fence_id = row
        .delete_fence_id
        .as_deref()
        .and_then(canonical_object_id)
        .ok_or_else(|| invalid_upstream_response("deletion scheduling"))?;
    let delete_generation = row
        .delete_generation
        .filter(|generation| *generation > 0)
        .ok_or_else(|| invalid_upstream_response("deletion scheduling"))?;
    let delete_not_before = row
        .delete_not_before
        .filter(|value| parse_timestamp(value).is_some())
        .ok_or_else(|| invalid_upstream_response("deletion scheduling"))?;

    Ok(Json(ScheduledAttachmentBackupDeletion {
        object_key: request.object_key,
        attachment_ref: request.attachment_ref,
        version_ref: request.version_ref,
        delete_request_id: request.delete_request_id,
        delete_fence_id,
        delete_generation,
        delete_not_before,
    }))
}

#[utoipa::path(
    post,
    path = "/attachment-backups/delete/cancel",
    tag = "sync",
    request_body = DeleteAttachmentBackupRequest,
    responses(
        (status = 200, description = "Exact deletion request canceled or durably prevented", body = CanceledAttachmentBackupDeletion),
        (status = 400, description = "Invalid deletion identity"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription or backup access required"),
        (status = 409, description = "Backup changed or deletion is already being collected"),
        (status = 502, description = "Backup service unavailable")
    )
)]
async fn cancel_attachment_backup_deletion(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(request): Json<DeleteAttachmentBackupRequest>,
) -> Result<Json<CanceledAttachmentBackupDeletion>> {
    require_pro(&auth)?;
    let owner_user_id = canonical_owner(&auth)?;
    let request = validate_delete_request(request, &owner_user_id)?;
    let row: CanceledDeletionRow = rpc_single(
        &state,
        "cancel_attachment_backup_deletion",
        &DeleteRpcRequest {
            p_owner_user_id: &owner_user_id,
            p_attachment_ref: &request.attachment_ref,
            p_version_ref: &request.version_ref,
            p_object_key: &request.object_key,
            p_delete_request_id: &request.delete_request_id,
        },
    )
    .await
    .map_err(map_cancel_deletion_error)?;

    if !matches!(row.outcome.as_str(), "cancelled" | "dependency_appeared")
        || row.object_key != request.object_key
    {
        return Err(invalid_upstream_response("deletion cancellation"));
    }
    Ok(Json(CanceledAttachmentBackupDeletion {
        object_key: request.object_key,
        attachment_ref: request.attachment_ref,
        version_ref: request.version_ref,
        delete_request_id: request.delete_request_id,
    }))
}

#[derive(Debug)]
enum RpcFailure {
    Empty,
    Rejected {
        status: StatusCode,
        code: Option<String>,
    },
    Unavailable,
}

async fn rpc_single<RequestBody, Row>(
    state: &AppState,
    function: &str,
    request: &RequestBody,
) -> std::result::Result<Row, RpcFailure>
where
    RequestBody: Serialize + ?Sized,
    Row: DeserializeOwned,
{
    let mut response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/{function}",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(BACKUP_RPC_TIMEOUT)
        .json(request)
        .send()
        .await
        .map_err(|_| {
            tracing::warn!(function, "Attachment backup ledger request failed");
            RpcFailure::Unavailable
        })?;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_BACKUP_RPC_RESPONSE_BYTES as u64)
    {
        tracing::warn!(function, %status, "Attachment backup ledger response was too large");
        return Err(RpcFailure::Unavailable);
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        tracing::warn!(
            function,
            "Attachment backup ledger response could not be read"
        );
        RpcFailure::Unavailable
    })? {
        if bytes.len().saturating_add(chunk.len()) > MAX_BACKUP_RPC_RESPONSE_BYTES {
            tracing::warn!(function, %status, "Attachment backup ledger response was too large");
            return Err(RpcFailure::Unavailable);
        }
        bytes.extend_from_slice(&chunk);
    }

    if !status.is_success() {
        let code = serde_json::from_slice::<PostgrestError>(&bytes)
            .ok()
            .map(|error| error.code);
        tracing::warn!(function, %status, ?code, "Attachment backup ledger request was rejected");
        return Err(RpcFailure::Rejected { status, code });
    }

    let mut rows = serde_json::from_slice::<Vec<Row>>(&bytes).map_err(|_| {
        tracing::warn!(function, "Attachment backup ledger response was invalid");
        RpcFailure::Unavailable
    })?;
    match rows.len() {
        0 => Err(RpcFailure::Empty),
        1 => Ok(rows.pop().expect("row count was checked")),
        row_count => {
            tracing::warn!(
                function,
                row_count,
                "Attachment backup ledger returned multiple rows"
            );
            Err(RpcFailure::Unavailable)
        }
    }
}

async fn read_backup_object(
    state: &AppState,
    owner_user_id: &str,
    object_key: &str,
) -> Result<BackupObjectRow> {
    let row: BackupObjectRow = rpc_single(
        state,
        "read_attachment_backup_by_key",
        &ObjectKeyRpcRequest {
            p_owner_user_id: owner_user_id,
            p_object_key: object_key,
        },
    )
    .await
    .map_err(map_backup_error)?;
    validate_backup_object_row(&row, owner_user_id, object_key)?;
    Ok(row)
}

fn map_reservation_error(error: RpcFailure) -> SyncError {
    match &error {
        RpcFailure::Rejected { code, .. } if code.as_deref() == Some("54000") => {
            SyncError::AttachmentBackupQuotaExceeded
        }
        _ => map_backup_error(error),
    }
}

fn map_deletion_error(error: RpcFailure) -> SyncError {
    match &error {
        RpcFailure::Rejected { code, .. } if code.as_deref() == Some("55000") => {
            SyncError::AttachmentBackupNotFound
        }
        _ => map_backup_error(error),
    }
}

fn map_cancel_deletion_error(error: RpcFailure) -> SyncError {
    match &error {
        RpcFailure::Rejected { code, .. } if code.as_deref() == Some("55006") => {
            SyncError::AttachmentBackupDeleteTooLate
        }
        _ => map_backup_error(error),
    }
}

fn map_backup_error(error: RpcFailure) -> SyncError {
    match error {
        RpcFailure::Empty => SyncError::AttachmentBackupNotFound,
        RpcFailure::Rejected { status, code }
            if status == StatusCode::UNAUTHORIZED
                || status == StatusCode::FORBIDDEN
                || code.as_deref() == Some("42501") =>
        {
            SyncError::AttachmentBackupForbidden
        }
        RpcFailure::Rejected { code, .. } if code.as_deref() == Some("22023") => invalid_request(),
        RpcFailure::Rejected { code, .. } if matches!(code.as_deref(), Some("40001" | "55000")) => {
            SyncError::AttachmentBackupConflict
        }
        RpcFailure::Rejected { .. } | RpcFailure::Unavailable => {
            SyncError::AttachmentBackupServiceUnavailable
        }
    }
}

fn require_pro(auth: &AuthContext) -> Result<()> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    Ok(())
}

fn canonical_owner(auth: &AuthContext) -> Result<String> {
    canonical_uuid(&auth.claims.sub).ok_or_else(|| {
        tracing::warn!("Authenticated attachment backup subject was not a canonical UUID");
        SyncError::AttachmentBackupForbidden
    })
}

fn canonical_uuid(value: &str) -> Option<String> {
    let uuid = Uuid::parse_str(value).ok()?;
    let canonical = uuid.to_string();
    (canonical == value).then_some(canonical)
}

fn canonical_object_id(value: &str) -> Option<String> {
    let uuid = Uuid::parse_str(value).ok()?;
    let canonical = uuid.to_string();
    (canonical == value && uuid.get_version() == Some(Version::Random)).then_some(canonical)
}

fn validate_ref(value: &str, kind: &str) -> Result<()> {
    if value.len() != 43
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(SyncError::BadRequest(format!(
            "Attachment backup {kind} reference is invalid"
        )));
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(invalid_request());
    }
    Ok(())
}

fn validate_object_key(value: &str, owner_user_id: &str) -> Result<String> {
    let (owner, filename) = value.split_once('/').ok_or_else(invalid_request)?;
    let object_id = filename.strip_suffix(".anb1").ok_or_else(invalid_request)?;
    let object_uuid = Uuid::parse_str(object_id).map_err(|_| invalid_request())?;
    if owner != owner_user_id
        || filename.contains('/')
        || object_uuid.to_string() != object_id
        || !matches!(
            object_uuid.get_version(),
            Some(Version::Random | Version::SortRand)
        )
    {
        return Err(invalid_request());
    }
    Ok(value.to_string())
}

fn validate_delete_request(
    request: DeleteAttachmentBackupRequest,
    owner_user_id: &str,
) -> Result<DeleteAttachmentBackupRequest> {
    validate_object_key(&request.object_key, owner_user_id)?;
    validate_ref(&request.attachment_ref, "attachment")?;
    validate_ref(&request.version_ref, "version")?;
    if request.attachment_ref == request.version_ref
        || canonical_uuid(&request.delete_request_id).is_none()
    {
        return Err(invalid_request());
    }
    Ok(request)
}

fn validate_deletion_row_identity(
    object_key: &str,
    delete_request_id: &str,
    request: &DeleteAttachmentBackupRequest,
    operation: &str,
) -> Result<()> {
    if object_key != request.object_key
        || delete_request_id != request.delete_request_id
        || canonical_uuid(delete_request_id).is_none()
    {
        return Err(invalid_upstream_response(operation));
    }
    Ok(())
}

fn validate_reserved_row(
    row: &ReservedRow,
    owner_user_id: &str,
    request: &ReserveAttachmentBackupRequest,
) -> Result<()> {
    canonical_object_id(&row.object_id).ok_or_else(|| invalid_upstream_response("reservation"))?;
    validate_object_key(&row.object_key, owner_user_id)
        .map_err(|_| invalid_upstream_response("reservation"))?;
    let size = valid_size(row.ciphertext_size_bytes)?;
    let reservation_expires_at = parse_timestamp(&row.reservation_expires_at)
        .ok_or_else(|| invalid_upstream_response("reservation"))?;
    let cleanup_not_before = parse_timestamp(&row.cleanup_not_before)
        .ok_or_else(|| invalid_upstream_response("reservation"))?;
    if row
        .ciphertext_sha256
        .as_deref()
        .is_some_and(|hash| validate_sha256(hash).is_err())
    {
        return Err(invalid_upstream_response("reservation"));
    }
    if size != request.ciphertext_size_bytes
        || row.format_version != request.format_version
        || !matches!(row.object_state.as_str(), "reserved" | "ready" | "current")
        || (row.object_state != "reserved" && row.ciphertext_sha256.is_none())
        || cleanup_not_before < reservation_expires_at
    {
        return Err(invalid_upstream_response("reservation"));
    }
    Ok(())
}

fn validate_marked_signed_row(
    marked: &MarkedSignedRow,
    reserved: &BackupObjectRow,
    expected_ciphertext_sha256: &str,
    requested_upload_expiry: &str,
) -> Result<()> {
    let requested_upload_expiry = parse_timestamp(requested_upload_expiry)
        .ok_or_else(|| invalid_upstream_response("upload signing"))?;
    let last_signed_at = parse_timestamp(&marked.last_signed_at)
        .ok_or_else(|| invalid_upstream_response("upload signing"))?;
    let upload_expires_at = parse_timestamp(&marked.upload_expires_at)
        .ok_or_else(|| invalid_upstream_response("upload signing"))?;
    let cleanup_not_before = parse_timestamp(&marked.cleanup_not_before)
        .ok_or_else(|| invalid_upstream_response("upload signing"))?;
    let minimum_cleanup = upload_expires_at
        .checked_add_signed(TimeDelta::seconds(UPLOAD_CLEANUP_GRACE_SECONDS))
        .ok_or_else(|| invalid_upstream_response("upload signing"))?;
    if marked.object_id != reserved.object_id
        || marked.object_key != reserved.object_key
        || marked.ciphertext_sha256 != expected_ciphertext_sha256
        || last_signed_at >= upload_expires_at
        || upload_expires_at < requested_upload_expiry
        || cleanup_not_before < minimum_cleanup
    {
        return Err(invalid_upstream_response("upload signing"));
    }
    Ok(())
}

fn validate_backup_object_row(
    row: &BackupObjectRow,
    owner_user_id: &str,
    expected_object_key: &str,
) -> Result<()> {
    canonical_object_id(&row.object_id)
        .ok_or_else(|| invalid_upstream_response("object lookup"))?;
    let object_key = validate_object_key(&row.object_key, owner_user_id)
        .map_err(|_| invalid_upstream_response("object lookup"))?;
    validate_ref(&row.attachment_ref, "attachment")
        .map_err(|_| invalid_upstream_response("object lookup"))?;
    validate_ref(&row.version_ref, "version")
        .map_err(|_| invalid_upstream_response("object lookup"))?;
    valid_size(row.ciphertext_size_bytes)?;
    let reservation_expires_at = parse_timestamp(&row.reservation_expires_at)
        .ok_or_else(|| invalid_upstream_response("object lookup"))?;
    let cleanup_not_before = parse_timestamp(&row.cleanup_not_before)
        .ok_or_else(|| invalid_upstream_response("object lookup"))?;
    let upload_expires_at = match row.upload_expires_at.as_deref() {
        Some(value) => {
            Some(parse_timestamp(value).ok_or_else(|| invalid_upstream_response("object lookup"))?)
        }
        None => None,
    };
    if row
        .ciphertext_sha256
        .as_deref()
        .is_some_and(|hash| validate_sha256(hash).is_err())
    {
        return Err(invalid_upstream_response("object lookup"));
    }
    if object_key != expected_object_key
        || row.format_version != FORMAT_VERSION
        || row.attachment_ref == row.version_ref
        || !matches!(
            row.object_state.as_str(),
            "reserved" | "ready" | "current" | "deleting"
        )
        || (matches!(row.object_state.as_str(), "ready" | "current")
            && row.ciphertext_sha256.is_none())
        || cleanup_not_before < reservation_expires_at
        || upload_expires_at.is_some_and(|expiry| cleanup_not_before < expiry)
    {
        return Err(invalid_upstream_response("object lookup"));
    }
    Ok(())
}

fn validate_promoted_row(
    row: &PromotedRow,
    owner_user_id: &str,
    candidate: &BackupObjectRow,
    expected_current_object_key: Option<&str>,
) -> Result<()> {
    let current_id = canonical_object_id(&row.current_object_id)
        .ok_or_else(|| invalid_upstream_response("promotion"))?;
    let current_key = validate_object_key(&row.current_object_key, owner_user_id)
        .map_err(|_| invalid_upstream_response("promotion"))?;
    let displaced = match (&row.displaced_object_id, &row.displaced_object_key) {
        (None, None) => None,
        (Some(id), Some(key)) => {
            canonical_object_id(id).ok_or_else(|| invalid_upstream_response("promotion"))?;
            Some(
                validate_object_key(key, owner_user_id)
                    .map_err(|_| invalid_upstream_response("promotion"))?,
            )
        }
        _ => return Err(invalid_upstream_response("promotion")),
    };
    if current_id != candidate.object_id
        || current_key != candidate.object_key
        || row.current_version_ref != candidate.version_ref
        || candidate.ciphertext_sha256.as_deref() != Some(&row.current_ciphertext_sha256)
        || validate_sha256(&row.current_ciphertext_sha256).is_err()
        || (row.was_promoted && displaced.as_deref() != expected_current_object_key)
        || (!row.was_promoted && displaced.is_some())
    {
        return Err(invalid_upstream_response("promotion"));
    }
    Ok(())
}

fn validate_prepared_download(
    row: &PreparedDownloadRow,
    owner_user_id: &str,
    expected_object_key: &str,
    requested_expiry: &str,
) -> Result<()> {
    canonical_object_id(&row.object_id)
        .ok_or_else(|| invalid_upstream_response("download preparation"))?;
    let object_key = validate_object_key(&row.object_key, owner_user_id)
        .map_err(|_| invalid_upstream_response("download preparation"))?;
    valid_size(row.ciphertext_size_bytes)?;
    validate_sha256(&row.ciphertext_sha256)
        .map_err(|_| invalid_upstream_response("download preparation"))?;
    let cleanup_not_before = parse_timestamp(&row.cleanup_not_before)
        .ok_or_else(|| invalid_upstream_response("download preparation"))?;
    let requested_expiry = parse_timestamp(requested_expiry)
        .ok_or_else(|| invalid_upstream_response("download preparation"))?;
    let minimum_cleanup = requested_expiry
        .checked_add_signed(TimeDelta::seconds(DOWNLOAD_CLEANUP_GRACE_SECONDS))
        .ok_or_else(|| invalid_upstream_response("download preparation"))?;
    if object_key != expected_object_key
        || row.format_version != FORMAT_VERSION
        || cleanup_not_before < minimum_cleanup
    {
        return Err(invalid_upstream_response("download preparation"));
    }
    Ok(())
}

fn valid_size(value: i64) -> Result<u64> {
    let value = u64::try_from(value)
        .ok()
        .filter(|value| (1..=MAX_CIPHERTEXT_SIZE_BYTES).contains(value))
        .ok_or_else(|| invalid_upstream_response("ciphertext size"))?;
    Ok(value)
}

fn parse_timestamp(value: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    chrono::DateTime::parse_from_rfc3339(value).ok()
}

fn future_timestamp(seconds: i64) -> Result<String> {
    let delta = TimeDelta::try_seconds(seconds)
        .ok_or_else(|| SyncError::Internal("Attachment backup expiry is invalid".to_string()))?;
    Utc::now()
        .checked_add_signed(delta)
        .ok_or_else(|| SyncError::Internal("Attachment backup expiry is invalid".to_string()))
        .map(|expiry| expiry.to_rfc3339_opts(SecondsFormat::Secs, true))
}

fn invalid_request() -> SyncError {
    SyncError::BadRequest("Attachment backup request is invalid".to_string())
}

fn invalid_upstream_response(operation: &str) -> SyncError {
    tracing::warn!(
        operation,
        "Attachment backup ledger response failed validation"
    );
    SyncError::AttachmentBackupServiceUnavailable
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode, header},
    };
    use hypr_api_auth::{AuthContext, Claims};
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header as request_header, method, path},
    };

    use super::*;
    use crate::SyncConfig;

    const OWNER: &str = "11111111-1111-4111-8111-111111111111";
    const OBJECT_ID: &str = "22222222-2222-4222-8222-222222222222";
    const OBJECT_UUID: &str = "33333333-3333-4333-8333-333333333333";
    const DELETE_REQUEST_ID: &str = "44444444-4444-4444-8444-444444444444";
    const DISPLACED_UUID: &str = "55555555-5555-4555-8555-555555555555";
    const DELETE_FENCE_ID: &str = "66666666-6666-4666-8666-666666666666";
    const ATTACHMENT_REF: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const VERSION_REF: &str = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
    const CIPHERTEXT_SHA256: &str =
        "ad47fd9e87159d651a53b3dfba3ef200684a9ed88c2528b62e18f3881fe203b0";

    fn object_key() -> String {
        format!("{OWNER}/{OBJECT_UUID}.anb1")
    }

    fn displaced_key() -> String {
        format!("{OWNER}/{DISPLACED_UUID}.anb1")
    }

    fn timestamp_after(seconds: i64) -> String {
        (Utc::now() + TimeDelta::seconds(seconds)).to_rfc3339_opts(SecondsFormat::Secs, true)
    }

    fn test_state(server: &MockServer) -> AppState {
        AppState::new(SyncConfig {
            project_url: server.uri(),
            token_issuer_api_key: "issuer-key".to_string(),
            database_id: "database-id".to_string(),
            token_ttl_seconds: 60,
            supabase_url: server.uri(),
            supabase_anon_key: "anon-key".to_string(),
            supabase_service_role_key: "service-role-key".to_string(),
        })
    }

    fn test_router_with_state(state: AppState, is_pro: bool) -> axum::Router {
        router().with_state(state).layer(Extension(AuthContext {
            token: "user-token".to_string(),
            claims: Claims {
                sub: OWNER.to_string(),
                email: None,
                entitlements: is_pro
                    .then(|| "hyprnote_pro".to_string())
                    .into_iter()
                    .collect(),
                subscription_status: None,
                trial_end: None,
                has_payment_method: None,
            },
        }))
    }

    fn test_router(server: &MockServer, is_pro: bool) -> axum::Router {
        test_router_with_state(test_state(server), is_pro)
    }

    fn json_request(method: Method, path: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn reserved_row(state: &str, hash: Option<&str>) -> Value {
        json!({
            "object_id": OBJECT_ID,
            "object_key": object_key(),
            "object_state": state,
            "ciphertext_sha256": hash,
            "ciphertext_size_bytes": 1234,
            "format_version": 1,
            "reservation_expires_at": timestamp_after(15 * 60),
            "cleanup_not_before": timestamp_after(30 * 60),
            "was_created": true
        })
    }

    fn object_row(state: &str, hash: Option<&str>) -> Value {
        json!({
            "object_id": OBJECT_ID,
            "attachment_ref": ATTACHMENT_REF,
            "version_ref": VERSION_REF,
            "object_key": object_key(),
            "object_state": state,
            "ciphertext_sha256": hash,
            "ciphertext_size_bytes": 1234,
            "format_version": 1,
            "reservation_expires_at": timestamp_after(15 * 60),
            "upload_expires_at": hash.map(|_| timestamp_after(2 * 60 * 60)),
            "cleanup_not_before": timestamp_after(26 * 60 * 60)
        })
    }

    fn delete_request_body() -> Value {
        json!({
            "objectKey": object_key(),
            "attachmentRef": ATTACHMENT_REF,
            "versionRef": VERSION_REF,
            "deleteRequestId": DELETE_REQUEST_ID
        })
    }

    fn scheduled_deletion_row(delete_not_before: &str, was_created: bool) -> Value {
        json!({
            "outcome": "scheduled",
            "object_id": OBJECT_ID,
            "object_key": object_key(),
            "delete_request_id": DELETE_REQUEST_ID,
            "delete_fence_id": DELETE_FENCE_ID,
            "delete_generation": 7,
            "delete_not_before": delete_not_before,
            "was_created": was_created
        })
    }

    async fn mount_rpc(server: &MockServer, function: &str, response: ResponseTemplate) {
        Mock::given(method("POST"))
            .and(path(format!("/rest/v1/rpc/{function}")))
            .and(request_header("apikey", "service-role-key"))
            .and(request_header("authorization", "Bearer service-role-key"))
            .respond_with(response)
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn reserves_identity_without_issuing_a_storage_capability() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "reserve_attachment_backup",
            ResponseTemplate::new(200).set_body_json(json!([reserved_row("reserved", None)])),
        )
        .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/reserve",
                json!({
                    "attachmentRef": ATTACHMENT_REF,
                    "versionRef": VERSION_REF,
                    "ciphertextSizeBytes": 1234,
                    "formatVersion": 1
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = response_json(response).await;
        assert_eq!(body["objectId"], OBJECT_ID);
        assert_eq!(body["ciphertextSha256"], Value::Null);
        assert!(body.get("uploadToken").is_none());
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].url.path(),
            "/rest/v1/rpc/reserve_attachment_backup"
        );
    }

    #[tokio::test]
    async fn reports_reservation_races_as_conflicts() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "reserve_attachment_backup",
            ResponseTemplate::new(409).set_body_json(json!({
                "code": "40001",
                "message": "reservation-secret-must-not-leak"
            })),
        )
        .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/reserve",
                json!({
                    "attachmentRef": ATTACHMENT_REF,
                    "versionRef": VERSION_REF,
                    "ciphertextSizeBytes": 1234,
                    "formatVersion": 1
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], "attachment_backup_conflict");
        assert!(!body.to_string().contains("reservation-secret"));
    }

    #[tokio::test]
    async fn reports_reservation_limit_as_conflict() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "reserve_attachment_backup",
            ResponseTemplate::new(409).set_body_json(json!({
                "code": "55000",
                "message": "reservation-limit-secret-must-not-leak"
            })),
        )
        .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/reserve",
                json!({
                    "attachmentRef": ATTACHMENT_REF,
                    "versionRef": VERSION_REF,
                    "ciphertextSizeBytes": 1234,
                    "formatVersion": 1
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], "attachment_backup_conflict");
        assert!(!body.to_string().contains("reservation-limit-secret"));
    }

    #[tokio::test]
    async fn reads_then_marks_integrity_before_signing_upload() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200).set_body_json(json!([object_row("reserved", None)])),
        )
        .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/mark_attachment_backup_signed"))
            .and(body_partial_json(json!({
                "p_owner_user_id": OWNER,
                "p_object_id": OBJECT_ID,
                "p_ciphertext_sha256": CIPHERTEXT_SHA256
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "object_id": OBJECT_ID,
                "object_key": object_key(),
                "ciphertext_sha256": CIPHERTEXT_SHA256,
                "last_signed_at": timestamp_after(-1),
                "upload_expires_at": timestamp_after(2 * 60 * 60 + 30),
                "cleanup_not_before": timestamp_after(27 * 60 * 60)
            }])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!(
                "/storage/v1/object/upload/sign/{ATTACHMENT_BACKUP_BUCKET}/{}",
                object_key()
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "url": format!(
                    "/object/upload/sign/{ATTACHMENT_BACKUP_BUCKET}/{}?token=upload-secret",
                    object_key()
                )
            })))
            .mount(&server)
            .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/upload-grant",
                json!({
                    "objectKey": object_key(),
                    "ciphertextSha256": CIPHERTEXT_SHA256
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["uploadToken"], "upload-secret");
        assert_eq!(body["ciphertextSha256"], CIPHERTEXT_SHA256);

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(
            requests[0].url.path(),
            "/rest/v1/rpc/read_attachment_backup_by_key"
        );
        assert_eq!(
            requests[1].url.path(),
            "/rest/v1/rpc/mark_attachment_backup_signed"
        );
        assert!(
            requests[2]
                .url
                .path()
                .contains("/storage/v1/object/upload/sign/")
        );
    }

    #[tokio::test]
    async fn never_issues_upload_token_for_an_unsafe_cleanup_window() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200).set_body_json(json!([object_row("reserved", None)])),
        )
        .await;
        mount_rpc(
            &server,
            "mark_attachment_backup_signed",
            ResponseTemplate::new(200).set_body_json(json!([{
                "object_id": OBJECT_ID,
                "object_key": object_key(),
                "ciphertext_sha256": CIPHERTEXT_SHA256,
                "last_signed_at": timestamp_after(-1),
                "upload_expires_at": timestamp_after(2 * 60 * 60 + 4 * 60),
                "cleanup_not_before": timestamp_after(26 * 60 * 60 + 6 * 60)
            }])),
        )
        .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/upload-grant",
                json!({
                    "objectKey": object_key(),
                    "ciphertextSha256": CIPHERTEXT_SHA256
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
        assert!(
            !requests
                .iter()
                .any(|request| request.url.path().contains("/storage/v1/"))
        );
    }

    #[tokio::test]
    async fn never_issues_upload_token_when_integrity_mark_fails() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200).set_body_json(json!([object_row("reserved", None)])),
        )
        .await;
        mount_rpc(
            &server,
            "mark_attachment_backup_signed",
            ResponseTemplate::new(500).set_body_json(json!({
                "code": "XX000",
                "message": "database-secret-must-not-leak"
            })),
        )
        .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/upload-grant",
                json!({
                    "objectKey": object_key(),
                    "ciphertextSha256": CIPHERTEXT_SHA256
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = response_json(response).await.to_string();
        assert!(!body.contains("database-secret"));
        assert!(!body.contains("upload-secret"));
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
        assert!(
            !requests
                .iter()
                .any(|request| request.url.path().contains("/storage/v1/"))
        );
    }

    #[tokio::test]
    async fn reports_integrity_mark_races_as_conflicts_without_signing() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200).set_body_json(json!([object_row("reserved", None)])),
        )
        .await;
        mount_rpc(
            &server,
            "mark_attachment_backup_signed",
            ResponseTemplate::new(409).set_body_json(json!({
                "code": "40001",
                "message": "hash-secret-must-not-leak"
            })),
        )
        .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/upload-grant",
                json!({
                    "objectKey": object_key(),
                    "ciphertextSha256": CIPHERTEXT_SHA256
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], "attachment_backup_conflict");
        assert!(!body.to_string().contains("hash-secret"));
        assert!(
            !server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .any(|request| request.url.path().contains("/storage/v1/"))
        );
    }

    #[tokio::test]
    async fn verifies_storage_metadata_before_finalizing() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200)
                .set_body_json(json!([object_row("reserved", Some(CIPHERTEXT_SHA256))])),
        )
        .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/info/{ATTACHMENT_BACKUP_BUCKET}/{}",
                object_key()
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "size": 1234,
                "content_type": "application/octet-stream",
                "metadata": {
                    "ciphertextSha256": CIPHERTEXT_SHA256,
                    "formatVersion": 1
                }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/authenticated/{ATTACHMENT_BACKUP_BUCKET}/{}",
                object_key()
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0_u8; 1_234]))
            .mount(&server)
            .await;
        mount_rpc(
            &server,
            "finalize_attachment_backup",
            ResponseTemplate::new(200).set_body_json(json!([{
                "object_id": OBJECT_ID,
                "object_key": object_key(),
                "object_state": "ready",
                "was_finalized": true
            }])),
        )
        .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/finalize",
                json!({ "objectKey": object_key() }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_json(response).await["objectState"], "ready");
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests[0].url.path(),
            "/rest/v1/rpc/read_attachment_backup_by_key"
        );
        assert!(requests[1].url.path().contains("/storage/v1/object/info/"));
        assert!(
            requests[2]
                .url
                .path()
                .contains("/storage/v1/object/authenticated/")
        );
        assert_eq!(
            requests[3].url.path(),
            "/rest/v1/rpc/finalize_attachment_backup"
        );
    }

    #[tokio::test]
    async fn rejects_expired_finalize_before_storage_verification() {
        let server = MockServer::start().await;
        let mut row = object_row("reserved", Some(CIPHERTEXT_SHA256));
        row["reservation_expires_at"] = json!(timestamp_after(-3 * 60 * 60));
        row["upload_expires_at"] = json!(timestamp_after(-2 * 60 * 60));
        row["cleanup_not_before"] = json!(timestamp_after(-60 * 60));
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200).set_body_json(json!([row])),
        )
        .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/finalize",
                json!({ "objectKey": object_key() }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(response).await["error"]["code"],
            "attachment_backup_conflict"
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn accepts_finalize_retry_after_the_candidate_is_current() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200)
                .set_body_json(json!([object_row("current", Some(CIPHERTEXT_SHA256))])),
        )
        .await;
        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/finalize",
                json!({ "objectKey": object_key() }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["objectState"], "current");
        assert_eq!(body["wasFinalized"], false);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn rejects_storage_integrity_mismatch_without_finalizing() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200)
                .set_body_json(json!([object_row("reserved", Some(CIPHERTEXT_SHA256))])),
        )
        .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/info/{ATTACHMENT_BACKUP_BUCKET}/{}",
                object_key()
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "size": 1234,
                "content_type": "application/octet-stream",
                "metadata": {
                    "ciphertextSha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "formatVersion": 1
                }
            })))
            .mount(&server)
            .await;
        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/finalize",
                json!({ "objectKey": object_key() }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn rejects_storage_content_mismatch_without_finalizing() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200)
                .set_body_json(json!([object_row("reserved", Some(CIPHERTEXT_SHA256))])),
        )
        .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/info/{ATTACHMENT_BACKUP_BUCKET}/{}",
                object_key()
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "size": 1234,
                "content_type": "application/octet-stream",
                "metadata": {
                    "ciphertextSha256": CIPHERTEXT_SHA256,
                    "formatVersion": 1
                }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/authenticated/{ATTACHMENT_BACKUP_BUCKET}/{}",
                object_key()
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1_u8; 1_234]))
            .mount(&server)
            .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/finalize",
                json!({ "objectKey": object_key() }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(server.received_requests().await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn throttles_parallel_storage_content_verification() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200)
                .set_body_json(json!([object_row("reserved", Some(CIPHERTEXT_SHA256))])),
        )
        .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/info/{ATTACHMENT_BACKUP_BUCKET}/{}",
                object_key()
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "size": 1234,
                "content_type": "application/octet-stream",
                "metadata": {
                    "ciphertextSha256": CIPHERTEXT_SHA256,
                    "formatVersion": 1
                }
            })))
            .mount(&server)
            .await;
        let state = test_state(&server);
        let _verification_slot = state
            .attachment_verification_slots
            .clone()
            .try_acquire_owned()
            .unwrap();

        let response = test_router_with_state(state, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/finalize",
                json!({ "objectKey": object_key() }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn rejects_spoofed_storage_metadata_when_object_bytes_do_not_match() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200)
                .set_body_json(json!([object_row("reserved", Some(CIPHERTEXT_SHA256))])),
        )
        .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/info/{ATTACHMENT_BACKUP_BUCKET}/{}",
                object_key()
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "metadata": { "size": 1234, "mimetype": "application/octet-stream" },
                "user_metadata": {
                    "ciphertextSha256": CIPHERTEXT_SHA256,
                    "formatVersion": 1
                }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/authenticated/{ATTACHMENT_BACKUP_BUCKET}/{}",
                object_key()
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1; 1234]))
            .mount(&server)
            .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/finalize",
                json!({ "objectKey": object_key() }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(server.received_requests().await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn reports_storage_verification_failures_as_unavailable() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200)
                .set_body_json(json!([object_row("reserved", Some(CIPHERTEXT_SHA256))])),
        )
        .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/info/{ATTACHMENT_BACKUP_BUCKET}/{}",
                object_key()
            )))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_json(json!({ "message": "storage-secret-must-not-leak" })),
            )
            .mount(&server)
            .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/finalize",
                json!({ "objectKey": object_key() }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert!(
            !response_json(response)
                .await
                .to_string()
                .contains("storage-secret")
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn reports_head_cas_conflict_without_leaking_upstream_details() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200)
                .set_body_json(json!([object_row("ready", Some(CIPHERTEXT_SHA256))])),
        )
        .await;
        mount_rpc(
            &server,
            "promote_attachment_backup",
            ResponseTemplate::new(409).set_body_json(json!({
                "code": "40001",
                "message": "current-key-secret"
            })),
        )
        .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::PUT,
                "/attachment-backups/head",
                json!({
                    "objectKey": object_key(),
                    "expectedCurrentObjectKey": displaced_key()
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], "attachment_backup_conflict");
        assert!(!body.to_string().contains("current-key-secret"));
    }

    #[tokio::test]
    async fn returns_version_and_integrity_for_current_and_promoted_heads() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_current_attachment_backup",
            ResponseTemplate::new(200).set_body_json(json!([{
                "object_id": OBJECT_ID,
                "version_ref": VERSION_REF,
                "object_key": object_key(),
                "ciphertext_sha256": CIPHERTEXT_SHA256,
                "ciphertext_size_bytes": 1234,
                "format_version": 1
            }])),
        )
        .await;
        let current = test_router(&server, true)
            .oneshot(
                Request::get(format!("/attachment-backups/head/{ATTACHMENT_REF}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(current.status(), StatusCode::OK);
        let body = response_json(current).await;
        assert_eq!(body["versionRef"], VERSION_REF);
        assert_eq!(body["ciphertextSha256"], CIPHERTEXT_SHA256);

        let promote_server = MockServer::start().await;
        mount_rpc(
            &promote_server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200)
                .set_body_json(json!([object_row("ready", Some(CIPHERTEXT_SHA256))])),
        )
        .await;
        mount_rpc(
            &promote_server,
            "promote_attachment_backup",
            ResponseTemplate::new(200).set_body_json(json!([{
                "current_object_id": OBJECT_ID,
                "current_object_key": object_key(),
                "current_version_ref": VERSION_REF,
                "current_ciphertext_sha256": CIPHERTEXT_SHA256,
                "displaced_object_id": null,
                "displaced_object_key": null,
                "was_promoted": true
            }])),
        )
        .await;
        let promoted = test_router(&promote_server, true)
            .oneshot(json_request(
                Method::PUT,
                "/attachment-backups/head",
                json!({ "objectKey": object_key(), "expectedCurrentObjectKey": null }),
            ))
            .await
            .unwrap();
        assert_eq!(promoted.status(), StatusCode::OK);
        let body = response_json(promoted).await;
        assert_eq!(body["currentVersionRef"], VERSION_REF);
        assert_eq!(body["currentCiphertextSha256"], CIPHERTEXT_SHA256);
    }

    #[tokio::test]
    async fn prepares_current_download_before_signing() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "prepare_attachment_backup_download",
            ResponseTemplate::new(200).set_body_json(json!([{
                "object_id": OBJECT_ID,
                "object_key": object_key(),
                "ciphertext_sha256": CIPHERTEXT_SHA256,
                "ciphertext_size_bytes": 1234,
                "format_version": 1,
                "cleanup_not_before": timestamp_after(60 * 60)
            }])),
        )
        .await;
        Mock::given(method("POST"))
            .and(path(format!(
                "/storage/v1/object/sign/{ATTACHMENT_BACKUP_BUCKET}/{}",
                object_key()
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "signedURL": format!(
                    "/object/sign/{ATTACHMENT_BACKUP_BUCKET}/{}?token=download-secret",
                    object_key()
                )
            })))
            .mount(&server)
            .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/download",
                json!({ "objectKey": object_key() }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["objectId"], OBJECT_ID);
        assert_eq!(body["ciphertextSha256"], CIPHERTEXT_SHA256);
        assert!(
            body["signedUrl"]
                .as_str()
                .unwrap()
                .contains("download-secret")
        );
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0].url.path(),
            "/rest/v1/rpc/prepare_attachment_backup_download"
        );
        assert!(requests[1].url.path().contains("/storage/v1/object/sign/"));
    }

    #[tokio::test]
    async fn never_signs_a_download_for_an_unsafe_cleanup_window() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "prepare_attachment_backup_download",
            ResponseTemplate::new(200).set_body_json(json!([{
                "object_id": OBJECT_ID,
                "object_key": object_key(),
                "ciphertext_sha256": CIPHERTEXT_SHA256,
                "ciphertext_size_bytes": 1234,
                "format_version": 1,
                "cleanup_not_before": timestamp_after(DOWNLOAD_URL_TTL_SECONDS + 60)
            }])),
        )
        .await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/download",
                json!({ "objectKey": object_key() }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(
            !requests
                .iter()
                .any(|request| request.url.path().contains("/storage/v1/"))
        );
    }

    #[tokio::test]
    async fn never_signs_a_noncurrent_download() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "prepare_attachment_backup_download",
            ResponseTemplate::new(409).set_body_json(json!({
                "code": "55000",
                "message": "not current"
            })),
        )
        .await;
        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/download",
                json!({ "objectKey": object_key() }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn schedules_exact_delete_identity_with_stable_replay_shape_without_storage() {
        let delete_not_before = timestamp_after(24 * 60 * 60);
        let created_server = MockServer::start().await;
        mount_rpc(
            &created_server,
            "schedule_attachment_backup_deletion",
            ResponseTemplate::new(200)
                .set_body_json(json!([scheduled_deletion_row(&delete_not_before, true)])),
        )
        .await;

        let created_response = test_router(&created_server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/delete",
                delete_request_body(),
            ))
            .await
            .unwrap();
        assert_eq!(created_response.status(), StatusCode::OK);
        let created_body = response_json(created_response).await;
        assert_eq!(
            created_body,
            json!({
                "objectKey": object_key(),
                "attachmentRef": ATTACHMENT_REF,
                "versionRef": VERSION_REF,
                "deleteRequestId": DELETE_REQUEST_ID,
                "deleteFenceId": DELETE_FENCE_ID,
                "deleteGeneration": 7,
                "deleteNotBefore": delete_not_before
            })
        );
        let created_requests = created_server.received_requests().await.unwrap();
        assert_eq!(created_requests.len(), 1);
        assert_eq!(
            created_requests[0].url.path(),
            "/rest/v1/rpc/schedule_attachment_backup_deletion"
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&created_requests[0].body).unwrap(),
            json!({
                "p_owner_user_id": OWNER,
                "p_attachment_ref": ATTACHMENT_REF,
                "p_version_ref": VERSION_REF,
                "p_object_key": object_key(),
                "p_delete_request_id": DELETE_REQUEST_ID
            })
        );

        let replay_server = MockServer::start().await;
        mount_rpc(
            &replay_server,
            "schedule_attachment_backup_deletion",
            ResponseTemplate::new(200)
                .set_body_json(json!([scheduled_deletion_row(&delete_not_before, false)])),
        )
        .await;
        let replay_response = test_router(&replay_server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/delete",
                delete_request_body(),
            ))
            .await
            .unwrap();
        assert_eq!(replay_response.status(), StatusCode::OK);
        assert_eq!(response_json(replay_response).await, created_body);
        let replay_requests = replay_server.received_requests().await.unwrap();
        assert_eq!(replay_requests.len(), 1);
        assert!(
            replay_requests
                .iter()
                .all(|request| !request.url.path().contains("/storage/v1/"))
        );

        let dependency_server = MockServer::start().await;
        mount_rpc(
            &dependency_server,
            "cancel_attachment_backup_deletion",
            ResponseTemplate::new(200).set_body_json(json!([{
                "outcome": "dependency_appeared",
                "object_key": object_key(),
                "was_cancelled": false
            }])),
        )
        .await;
        let dependency = test_router(&dependency_server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/delete/cancel",
                delete_request_body(),
            ))
            .await
            .unwrap();
        assert_eq!(dependency.status(), StatusCode::OK);
        assert_eq!(
            response_json(dependency).await,
            json!({
                "objectKey": object_key(),
                "attachmentRef": ATTACHMENT_REF,
                "versionRef": VERSION_REF,
                "deleteRequestId": DELETE_REQUEST_ID
            })
        );
        let dependency_requests = dependency_server.received_requests().await.unwrap();
        assert_eq!(dependency_requests.len(), 1);
        assert!(
            dependency_requests
                .iter()
                .all(|request| !request.url.path().contains("/storage/v1/"))
        );
    }

    #[tokio::test]
    async fn distinguishes_delete_outcomes_from_generic_conflicts_without_storage() {
        let dependency_server = MockServer::start().await;
        mount_rpc(
            &dependency_server,
            "schedule_attachment_backup_deletion",
            ResponseTemplate::new(200).set_body_json(json!([{
                "outcome": "dependency_appeared",
                "object_id": null,
                "object_key": object_key(),
                "delete_request_id": DELETE_REQUEST_ID,
                "delete_fence_id": null,
                "delete_generation": null,
                "delete_not_before": null,
                "was_created": false
            }])),
        )
        .await;
        let dependency = test_router(&dependency_server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/delete",
                delete_request_body(),
            ))
            .await
            .unwrap();
        assert_eq!(dependency.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(dependency).await["error"]["code"],
            "attachment_backup_dependency_appeared"
        );
        let dependency_requests = dependency_server.received_requests().await.unwrap();
        assert_eq!(dependency_requests.len(), 1);
        assert!(
            dependency_requests
                .iter()
                .all(|request| !request.url.path().contains("/storage/v1/"))
        );

        let cancelled_server = MockServer::start().await;
        mount_rpc(
            &cancelled_server,
            "schedule_attachment_backup_deletion",
            ResponseTemplate::new(200).set_body_json(json!([{
                "outcome": "cancelled",
                "object_id": null,
                "object_key": object_key(),
                "delete_request_id": DELETE_REQUEST_ID,
                "delete_fence_id": null,
                "delete_generation": null,
                "delete_not_before": null,
                "was_created": false
            }])),
        )
        .await;
        let cancelled = test_router(&cancelled_server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/delete",
                delete_request_body(),
            ))
            .await
            .unwrap();
        assert_eq!(cancelled.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(cancelled).await["error"]["code"],
            "attachment_backup_delete_cancelled"
        );
        let cancelled_requests = cancelled_server.received_requests().await.unwrap();
        assert_eq!(cancelled_requests.len(), 1);
        assert!(
            cancelled_requests
                .iter()
                .all(|request| !request.url.path().contains("/storage/v1/"))
        );

        let conflict_server = MockServer::start().await;
        mount_rpc(
            &conflict_server,
            "schedule_attachment_backup_deletion",
            ResponseTemplate::new(409).set_body_json(json!({
                "code": "40001",
                "message": "delete identity changed"
            })),
        )
        .await;
        let conflict = test_router(&conflict_server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/delete",
                delete_request_body(),
            ))
            .await
            .unwrap();
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(conflict).await["error"]["code"],
            "attachment_backup_conflict"
        );
        let conflict_requests = conflict_server.received_requests().await.unwrap();
        assert_eq!(conflict_requests.len(), 1);
        assert!(
            conflict_requests
                .iter()
                .all(|request| !request.url.path().contains("/storage/v1/"))
        );
    }

    #[tokio::test]
    async fn cancels_exact_delete_identity_with_a_stable_idempotent_shape() {
        let created_server = MockServer::start().await;
        mount_rpc(
            &created_server,
            "cancel_attachment_backup_deletion",
            ResponseTemplate::new(200).set_body_json(json!([{
                "outcome": "cancelled",
                "object_key": object_key(),
                "was_cancelled": true
            }])),
        )
        .await;
        let created = test_router(&created_server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/delete/cancel",
                delete_request_body(),
            ))
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::OK);
        let created_body = response_json(created).await;
        assert_eq!(
            created_body,
            json!({
                "objectKey": object_key(),
                "attachmentRef": ATTACHMENT_REF,
                "versionRef": VERSION_REF,
                "deleteRequestId": DELETE_REQUEST_ID
            })
        );
        let requests = created_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].url.path(),
            "/rest/v1/rpc/cancel_attachment_backup_deletion"
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
            json!({
                "p_owner_user_id": OWNER,
                "p_attachment_ref": ATTACHMENT_REF,
                "p_version_ref": VERSION_REF,
                "p_object_key": object_key(),
                "p_delete_request_id": DELETE_REQUEST_ID
            })
        );
        assert!(
            requests
                .iter()
                .all(|request| !request.url.path().contains("/storage/v1/"))
        );

        let replay_server = MockServer::start().await;
        mount_rpc(
            &replay_server,
            "cancel_attachment_backup_deletion",
            ResponseTemplate::new(200).set_body_json(json!([{
                "outcome": "cancelled",
                "object_key": object_key(),
                "was_cancelled": false
            }])),
        )
        .await;
        let replay = test_router(&replay_server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/delete/cancel",
                delete_request_body(),
            ))
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::OK);
        assert_eq!(response_json(replay).await, created_body);
        let replay_requests = replay_server.received_requests().await.unwrap();
        assert_eq!(replay_requests.len(), 1);
        assert!(
            replay_requests
                .iter()
                .all(|request| !request.url.path().contains("/storage/v1/"))
        );
    }

    #[tokio::test]
    async fn reports_delete_cancellation_after_gc_as_too_late() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "cancel_attachment_backup_deletion",
            ResponseTemplate::new(409).set_body_json(json!({
                "code": "55006",
                "message": "gc already owns the object"
            })),
        )
        .await;
        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/delete/cancel",
                delete_request_body(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(response).await["error"]["code"],
            "attachment_backup_delete_too_late"
        );
    }

    #[tokio::test]
    async fn requires_pro_and_rejects_bad_inputs_before_upstream_calls() {
        let server = MockServer::start().await;
        let non_pro = test_router(&server, false)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/download",
                json!({ "objectKey": object_key() }),
            ))
            .await
            .unwrap();
        assert_eq!(non_pro.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response_json(non_pro).await["error"]["code"],
            "subscription_required"
        );

        for path in [
            "/attachment-backups/delete",
            "/attachment-backups/delete/cancel",
        ] {
            let non_pro_delete = test_router(&server, false)
                .oneshot(json_request(Method::POST, path, delete_request_body()))
                .await
                .unwrap();
            assert_eq!(non_pro_delete.status(), StatusCode::FORBIDDEN);
            assert_eq!(
                response_json(non_pro_delete).await["error"]["code"],
                "subscription_required"
            );
        }

        let invalid_key = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/download",
                json!({ "objectKey": "../other-user/object" }),
            ))
            .await
            .unwrap();
        assert_eq!(invalid_key.status(), StatusCode::BAD_REQUEST);

        let invalid_hash = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/upload-grant",
                json!({ "objectKey": object_key(), "ciphertextSha256": "ABC" }),
            ))
            .await
            .unwrap();
        assert_eq!(invalid_hash.status(), StatusCode::BAD_REQUEST);

        let unknown_field = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/delete",
                json!({
                    "objectKey": object_key(),
                    "attachmentRef": ATTACHMENT_REF,
                    "versionRef": VERSION_REF,
                    "deleteRequestId": DELETE_REQUEST_ID,
                    "ownerUserId": "attacker"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(unknown_field.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let noncanonical_request_id = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/delete/cancel",
                json!({
                    "objectKey": object_key(),
                    "attachmentRef": ATTACHMENT_REF,
                    "versionRef": VERSION_REF,
                    "deleteRequestId": "{44444444-4444-4444-8444-444444444444}"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(noncanonical_request_id.status(), StatusCode::BAD_REQUEST);
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn rejects_oversized_or_origin_injecting_ledger_responses() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_current_attachment_backup",
            ResponseTemplate::new(200)
                .set_body_string("x".repeat(MAX_BACKUP_RPC_RESPONSE_BYTES + 1)),
        )
        .await;
        let oversized = test_router(&server, true)
            .oneshot(
                Request::get(format!("/attachment-backups/head/{ATTACHMENT_REF}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(oversized.status(), StatusCode::BAD_GATEWAY);

        let second_server = MockServer::start().await;
        mount_rpc(
            &second_server,
            "read_current_attachment_backup",
            ResponseTemplate::new(200).set_body_json(json!([{
                "object_id": OBJECT_ID,
                "version_ref": VERSION_REF,
                "object_key": "https://attacker.example/secret",
                "ciphertext_sha256": CIPHERTEXT_SHA256,
                "ciphertext_size_bytes": 1234,
                "format_version": 1
            }])),
        )
        .await;
        let malformed = test_router(&second_server, true)
            .oneshot(
                Request::get(format!("/attachment-backups/head/{ATTACHMENT_REF}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(malformed.status(), StatusCode::BAD_GATEWAY);
        assert!(
            !response_json(malformed)
                .await
                .to_string()
                .contains("attacker.example")
        );
    }
}

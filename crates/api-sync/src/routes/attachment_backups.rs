use anlg_api_auth::AuthContext;
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{HeaderValue, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post, put},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::OpenApi;

use crate::{
    error::{Result, SyncError},
    state::AppState,
};

mod ledger;
mod validation;

#[cfg(test)]
use ledger::MAX_BACKUP_RPC_RESPONSE_BYTES;
use ledger::{
    CanceledDeletionRow, CurrentRow, CurrentRpcRequest, DeleteRpcRequest, FinalizeRpcRequest,
    FinalizedRow, MarkSignedRpcRequest, MarkedSignedRow, PrepareDownloadRpcRequest,
    PreparedDownloadRow, PromoteRpcRequest, PromotedRow, ReserveRpcRequest, ReservedRow,
    ScheduledDeletionRow, map_backup_error, map_cancel_deletion_error, map_deletion_error,
    map_reservation_error, read_backup_object, rpc_single,
};
use validation::{
    canonical_object_id, canonical_owner, future_timestamp, invalid_request,
    invalid_upstream_response, parse_timestamp, require_pro, valid_size, validate_delete_request,
    validate_deletion_row_identity, validate_marked_signed_row, validate_object_key,
    validate_prepared_download, validate_promoted_row, validate_ref, validate_reserved_row,
    validate_sha256,
};

const ATTACHMENT_BACKUP_BUCKET: &str = "attachment-backups";
const MAX_BACKUP_REQUEST_BYTES: usize = 4 * 1024;
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

#[cfg(test)]
mod tests;

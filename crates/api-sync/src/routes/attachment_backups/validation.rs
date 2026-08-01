use anlg_api_auth::AuthContext;
use chrono::{SecondsFormat, TimeDelta, Utc};
use uuid::{Uuid, Version};

use super::ledger::{
    BackupObjectRow, MarkedSignedRow, PreparedDownloadRow, PromotedRow, ReservedRow,
};
use super::{
    DOWNLOAD_CLEANUP_GRACE_SECONDS, DeleteAttachmentBackupRequest, FORMAT_VERSION,
    MAX_CIPHERTEXT_SIZE_BYTES, ReserveAttachmentBackupRequest, UPLOAD_CLEANUP_GRACE_SECONDS,
};
use crate::error::{Result, SyncError};

pub(super) fn require_pro(auth: &AuthContext) -> Result<()> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    Ok(())
}

pub(super) fn canonical_owner(auth: &AuthContext) -> Result<String> {
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

pub(super) fn canonical_object_id(value: &str) -> Option<String> {
    let uuid = Uuid::parse_str(value).ok()?;
    let canonical = uuid.to_string();
    (canonical == value && uuid.get_version() == Some(Version::Random)).then_some(canonical)
}

pub(super) fn validate_ref(value: &str, kind: &str) -> Result<()> {
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

pub(super) fn validate_sha256(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(invalid_request());
    }
    Ok(())
}

pub(super) fn validate_object_key(value: &str, owner_user_id: &str) -> Result<String> {
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

pub(super) fn validate_delete_request(
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

pub(super) fn validate_deletion_row_identity(
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

pub(super) fn validate_reserved_row(
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

pub(super) fn validate_marked_signed_row(
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

pub(super) fn validate_backup_object_row(
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

pub(super) fn validate_promoted_row(
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

pub(super) fn validate_prepared_download(
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

pub(super) fn valid_size(value: i64) -> Result<u64> {
    let value = u64::try_from(value)
        .ok()
        .filter(|value| (1..=MAX_CIPHERTEXT_SIZE_BYTES).contains(value))
        .ok_or_else(|| invalid_upstream_response("ciphertext size"))?;
    Ok(value)
}

pub(super) fn parse_timestamp(value: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    chrono::DateTime::parse_from_rfc3339(value).ok()
}

pub(super) fn future_timestamp(seconds: i64) -> Result<String> {
    let delta = TimeDelta::try_seconds(seconds)
        .ok_or_else(|| SyncError::Internal("Attachment backup expiry is invalid".to_string()))?;
    Utc::now()
        .checked_add_signed(delta)
        .ok_or_else(|| SyncError::Internal("Attachment backup expiry is invalid".to_string()))
        .map(|expiry| expiry.to_rfc3339_opts(SecondsFormat::Secs, true))
}

pub(super) fn invalid_request() -> SyncError {
    SyncError::BadRequest("Attachment backup request is invalid".to_string())
}

pub(super) fn invalid_upstream_response(operation: &str) -> SyncError {
    tracing::warn!(
        operation,
        "Attachment backup ledger response failed validation"
    );
    SyncError::AttachmentBackupServiceUnavailable
}

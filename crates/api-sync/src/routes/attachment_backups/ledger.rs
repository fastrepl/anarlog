use std::time::Duration;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::validation::{invalid_request, validate_backup_object_row};
use crate::{
    error::{Result, SyncError},
    state::AppState,
};

const BACKUP_RPC_TIMEOUT: Duration = Duration::from_secs(5);
pub(super) const MAX_BACKUP_RPC_RESPONSE_BYTES: usize = 16 * 1024;

#[derive(Serialize)]
pub(super) struct ReserveRpcRequest<'a> {
    pub(super) p_owner_user_id: &'a str,
    pub(super) p_attachment_ref: &'a str,
    pub(super) p_version_ref: &'a str,
    pub(super) p_ciphertext_size_bytes: i64,
    pub(super) p_format_version: i16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReservedRow {
    pub(super) object_id: String,
    pub(super) object_key: String,
    pub(super) object_state: String,
    pub(super) ciphertext_size_bytes: i64,
    pub(super) format_version: i16,
    pub(super) reservation_expires_at: String,
    pub(super) cleanup_not_before: String,
    pub(super) ciphertext_sha256: Option<String>,
    pub(super) was_created: bool,
}

#[derive(Serialize)]
pub(super) struct MarkSignedRpcRequest<'a> {
    pub(super) p_owner_user_id: &'a str,
    pub(super) p_object_id: &'a str,
    pub(super) p_upload_expires_at: &'a str,
    pub(super) p_ciphertext_sha256: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MarkedSignedRow {
    pub(super) object_id: String,
    pub(super) object_key: String,
    pub(super) last_signed_at: String,
    pub(super) upload_expires_at: String,
    pub(super) cleanup_not_before: String,
    pub(super) ciphertext_sha256: String,
}

#[derive(Serialize)]
pub(super) struct ObjectKeyRpcRequest<'a> {
    pub(super) p_owner_user_id: &'a str,
    pub(super) p_object_key: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BackupObjectRow {
    pub(super) object_id: String,
    pub(super) attachment_ref: String,
    pub(super) version_ref: String,
    pub(super) object_key: String,
    pub(super) object_state: String,
    pub(super) ciphertext_size_bytes: i64,
    pub(super) format_version: i16,
    pub(super) reservation_expires_at: String,
    pub(super) upload_expires_at: Option<String>,
    pub(super) cleanup_not_before: String,
    pub(super) ciphertext_sha256: Option<String>,
}

#[derive(Serialize)]
pub(super) struct FinalizeRpcRequest<'a> {
    pub(super) p_owner_user_id: &'a str,
    pub(super) p_object_id: &'a str,
    pub(super) p_object_key: &'a str,
    pub(super) p_observed_ciphertext_size_bytes: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FinalizedRow {
    pub(super) object_id: String,
    pub(super) object_key: String,
    pub(super) object_state: String,
    pub(super) was_finalized: bool,
}

#[derive(Serialize)]
pub(super) struct PromoteRpcRequest<'a> {
    pub(super) p_owner_user_id: &'a str,
    pub(super) p_candidate_object_id: &'a str,
    pub(super) p_candidate_object_key: &'a str,
    pub(super) p_expected_current_object_key: Option<&'a str>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PromotedRow {
    pub(super) current_object_id: String,
    pub(super) current_object_key: String,
    pub(super) current_version_ref: String,
    pub(super) current_ciphertext_sha256: String,
    pub(super) displaced_object_id: Option<String>,
    pub(super) displaced_object_key: Option<String>,
    pub(super) was_promoted: bool,
}

#[derive(Serialize)]
pub(super) struct CurrentRpcRequest<'a> {
    pub(super) p_owner_user_id: &'a str,
    pub(super) p_attachment_ref: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CurrentRow {
    pub(super) object_id: String,
    pub(super) version_ref: String,
    pub(super) object_key: String,
    pub(super) ciphertext_sha256: String,
    pub(super) ciphertext_size_bytes: i64,
    pub(super) format_version: i16,
}

#[derive(Serialize)]
pub(super) struct PrepareDownloadRpcRequest<'a> {
    pub(super) p_owner_user_id: &'a str,
    pub(super) p_object_key: &'a str,
    pub(super) p_download_expires_at: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreparedDownloadRow {
    pub(super) object_id: String,
    pub(super) object_key: String,
    pub(super) ciphertext_sha256: String,
    pub(super) ciphertext_size_bytes: i64,
    pub(super) format_version: i16,
    pub(super) cleanup_not_before: String,
}

#[derive(Serialize)]
pub(super) struct DeleteRpcRequest<'a> {
    pub(super) p_owner_user_id: &'a str,
    pub(super) p_attachment_ref: &'a str,
    pub(super) p_version_ref: &'a str,
    pub(super) p_object_key: &'a str,
    pub(super) p_delete_request_id: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScheduledDeletionRow {
    pub(super) outcome: String,
    pub(super) object_id: Option<String>,
    pub(super) object_key: String,
    pub(super) delete_request_id: String,
    pub(super) delete_fence_id: Option<String>,
    pub(super) delete_generation: Option<i64>,
    pub(super) delete_not_before: Option<String>,
    #[serde(rename = "was_created")]
    pub(super) _was_created: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CanceledDeletionRow {
    pub(super) outcome: String,
    pub(super) object_key: String,
    #[serde(rename = "was_cancelled")]
    pub(super) _was_cancelled: bool,
}

#[derive(Deserialize)]
struct PostgrestError {
    code: String,
}

#[derive(Debug)]
pub(super) enum RpcFailure {
    Empty,
    Rejected {
        status: StatusCode,
        code: Option<String>,
    },
    Unavailable,
}

pub(super) async fn rpc_single<RequestBody, Row>(
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

pub(super) async fn read_backup_object(
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

pub(super) fn map_reservation_error(error: RpcFailure) -> SyncError {
    match &error {
        RpcFailure::Rejected { code, .. } if code.as_deref() == Some("54000") => {
            SyncError::AttachmentBackupQuotaExceeded
        }
        _ => map_backup_error(error),
    }
}

pub(super) fn map_deletion_error(error: RpcFailure) -> SyncError {
    match &error {
        RpcFailure::Rejected { code, .. } if code.as_deref() == Some("55000") => {
            SyncError::AttachmentBackupNotFound
        }
        _ => map_backup_error(error),
    }
}

pub(super) fn map_cancel_deletion_error(error: RpcFailure) -> SyncError {
    match &error {
        RpcFailure::Rejected { code, .. } if code.as_deref() == Some("55006") => {
            SyncError::AttachmentBackupDeleteTooLate
        }
        _ => map_backup_error(error),
    }
}

pub(super) fn map_backup_error(error: RpcFailure) -> SyncError {
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

use chrono::{SecondsFormat, TimeDelta, Utc};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use uuid::Uuid;

use super::{
    GatewayHandoffRow, GatewaySnapshotRow, MAX_HANDOFF_RESPONSE_BYTES, PreparedAttachmentRow,
    SHARED_ATTACHMENT_BUCKET, SHARED_NOTE_TIMEOUT, SharedAttachmentDownload, SharedNoteHandoff,
    SharedNoteSnapshot, SharedNotesState,
};
use crate::{
    error::{Result, SyncError},
    routes::validate_shared_attachments,
    snapshot::MAX_SNAPSHOT_BODY_BYTES,
};

pub(super) async fn attachment_rpc_single<RequestBody>(
    state: &SharedNotesState,
    function: &str,
    request: &RequestBody,
) -> Result<PreparedAttachmentRow>
where
    RequestBody: Serialize + ?Sized,
{
    rpc_single(state, function, request, MAX_HANDOFF_RESPONSE_BYTES)
        .await
        .map_err(|error| match error {
            SyncError::SharedNoteNotFound => SyncError::SharedAttachmentNotFound,
            SyncError::SnapshotServiceUnavailable => SyncError::SharedAttachmentServiceUnavailable,
            other => other,
        })
}

pub(super) async fn sign_attachment_download(
    state: &SharedNotesState,
    row: PreparedAttachmentRow,
    expected_attachment_id: &str,
    expires_at: &str,
    expires_in_seconds: i64,
) -> Result<SharedAttachmentDownload> {
    validate_prepared_attachment(&row, expected_attachment_id, expires_at)?;
    let signed_url = state
        .storage
        .create_signed_url(
            SHARED_ATTACHMENT_BUCKET,
            &row.object_key,
            expires_in_seconds as u64,
        )
        .await
        .map_err(|error| {
            tracing::warn!(%error, "shared attachment download signing failed");
            SyncError::SharedAttachmentServiceUnavailable
        })?;
    Ok(SharedAttachmentDownload {
        id: row.attachment_id,
        filename: row.filename,
        content_type: row.content_type,
        size_bytes: row.size_bytes as u64,
        sha256: row.sha256,
        signed_url,
        expires_at: expires_at.to_string(),
    })
}

pub(super) fn validate_prepared_attachment(
    row: &PreparedAttachmentRow,
    expected_attachment_id: &str,
    expires_at: &str,
) -> Result<()> {
    let share_id =
        canonical_uuid(&row.share_id).ok_or(SyncError::SharedAttachmentServiceUnavailable)?;
    if row.attachment_id != expected_attachment_id
        || canonical_uuid_v4(&row.attachment_id).is_none()
        || !is_valid_attachment_object_key(&row.object_key, &share_id, expected_attachment_id)
        || row.filename.is_empty()
        || row.filename.len() > 1024
        || row.filename.trim() != row.filename
        || row.filename.contains(['/', '\\'])
        || row.filename.chars().any(char::is_control)
        || !is_valid_content_type(&row.content_type)
        || !(1..=512 * 1024 * 1024).contains(&row.size_bytes)
        || row.sha256.len() != 64
        || !row
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        || row.access_version < 1
        || chrono::DateTime::parse_from_rfc3339(&row.cleanup_not_before).is_err()
        || chrono::DateTime::parse_from_rfc3339(expires_at).is_err()
    {
        return Err(SyncError::SharedAttachmentServiceUnavailable);
    }
    Ok(())
}

pub(super) fn is_valid_attachment_object_key(
    value: &str,
    share_id: &str,
    attachment_id: &str,
) -> bool {
    let mut parts = value.split('/');
    let Some(owner) = parts.next() else {
        return false;
    };
    let Some(share) = parts.next() else {
        return false;
    };
    let Some(filename) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && canonical_uuid(owner).as_deref() == Some(owner)
        && share == share_id
        && filename == format!("{attachment_id}.sna1")
}

pub(super) fn is_valid_content_type(value: &str) -> bool {
    value.len() <= 255
        && value == value.to_ascii_lowercase()
        && value
            .split_once('/')
            .is_some_and(|(kind, subtype)| !kind.is_empty() && !subtype.is_empty())
}

pub(super) fn future_timestamp(seconds: i64) -> Result<String> {
    Utc::now()
        .checked_add_signed(TimeDelta::seconds(seconds))
        .map(|value| value.to_rfc3339_opts(SecondsFormat::Millis, true))
        .ok_or_else(|| SyncError::Internal("shared attachment expiry overflow".to_string()))
}

pub(super) fn validate_handoff_lease(value: &str) -> Result<String> {
    let expires_at = chrono::DateTime::parse_from_rfc3339(value)
        .map_err(|_| invalid_gateway_response("handoff lease expiry"))?;
    if expires_at <= Utc::now() {
        return Err(invalid_gateway_response("handoff lease expiry"));
    }
    Ok(value.to_string())
}

pub(super) async fn rpc_single<RequestBody, Row>(
    state: &SharedNotesState,
    function: &str,
    request: &RequestBody,
    max_response_bytes: usize,
) -> Result<Row>
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
        .timeout(SHARED_NOTE_TIMEOUT)
        .json(request)
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "shared-note gateway request failed");
            SyncError::SnapshotServiceUnavailable
        })?;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > max_response_bytes as u64)
    {
        tracing::warn!(%status, "shared-note gateway response was too large");
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        tracing::warn!(%error, "shared-note gateway response could not be read");
        SyncError::SnapshotServiceUnavailable
    })? {
        if bytes.len().saturating_add(chunk.len()) > max_response_bytes {
            tracing::warn!(%status, "shared-note gateway response was too large");
            return Err(SyncError::SnapshotServiceUnavailable);
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        tracing::warn!(%status, "shared-note gateway request was rejected");
        return Err(SyncError::SnapshotServiceUnavailable);
    }

    let mut rows = serde_json::from_slice::<Vec<Row>>(&bytes).map_err(|error| {
        tracing::warn!(%error, "shared-note gateway response was invalid");
        SyncError::SnapshotServiceUnavailable
    })?;
    match rows.len() {
        0 => Err(SyncError::SharedNoteNotFound),
        1 => Ok(rows.pop().expect("row count was checked")),
        row_count => {
            tracing::warn!(row_count, "shared-note gateway returned multiple rows");
            Err(SyncError::SnapshotServiceUnavailable)
        }
    }
}

pub(super) fn validate_snapshot(
    row: GatewaySnapshotRow,
    expected_share_id: Option<&str>,
) -> Result<SharedNoteSnapshot> {
    let share_id =
        canonical_uuid(&row.share_id).ok_or_else(|| invalid_gateway_response("share"))?;
    let body_size = serde_json::to_vec(&row.body_json)
        .map_err(|_| invalid_gateway_response("body"))?
        .len();
    if expected_share_id.is_some_and(|expected| expected != share_id)
        || row.schema_version != 1
        || row.content_revision < 1
        || row.title.trim() != row.title
        || row.title.len() > 4096
        || row.body_json.get("type").and_then(Value::as_str) != Some("doc")
        || body_size > MAX_SNAPSHOT_BODY_BYTES
        || validate_shared_attachments(&row.attachments_json, None).is_err()
        || chrono::DateTime::parse_from_rfc3339(&row.published_at).is_err()
    {
        return Err(invalid_gateway_response("snapshot"));
    }

    Ok(SharedNoteSnapshot {
        share_id,
        schema_version: row.schema_version,
        content_revision: row.content_revision,
        title: row.title,
        body: row.body_json,
        attachments: row.attachments_json,
        lease_expires_at: None,
        published_at: row.published_at,
    })
}

pub(super) fn validate_handoff(row: GatewayHandoffRow) -> Result<SharedNoteHandoff> {
    let request_id =
        canonical_uuid_v4(&row.request_id).ok_or_else(|| invalid_gateway_response("handoff"))?;
    if chrono::DateTime::parse_from_rfc3339(&row.expires_at).is_err() {
        return Err(invalid_gateway_response("handoff expiry"));
    }

    Ok(SharedNoteHandoff {
        request_id,
        expires_at: row.expires_at,
    })
}

pub(super) fn invalid_gateway_response(field: &str) -> SyncError {
    tracing::warn!(field, "shared-note gateway response failed validation");
    SyncError::SnapshotServiceUnavailable
}

pub(super) fn canonical_uuid(value: &str) -> Option<String> {
    let parsed = Uuid::parse_str(value).ok()?;
    let canonical = parsed.hyphenated().to_string();
    (canonical == value).then_some(canonical)
}

pub(super) fn canonical_uuid_v4(value: &str) -> Option<String> {
    let parsed = Uuid::parse_str(value).ok()?;
    let canonical = parsed.hyphenated().to_string();
    (parsed.get_version_num() == 4 && canonical == value).then_some(canonical)
}

pub(super) fn is_valid_link_token(value: &str) -> bool {
    value.len() == 43
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub(super) fn is_valid_public_slug(value: &str) -> bool {
    value.len() == 34
        && value.starts_with("s_")
        && value[2..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

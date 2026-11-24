use super::*;

const SNAPSHOT_BUCKET: &str = "e2ee-snapshots";
const SNAPSHOT_DOWNLOAD_URL_TTL_SECONDS: u64 = 300;
const MAX_SNAPSHOT_BYTES: u64 = 545_259_520;

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDescriptor {
    snapshot_sequence: u64,
    format_version: u16,
    payload_bytes: u64,
    payload_sha256: String,
    record_count: u32,
    key_id: String,
    head_sequence: u64,
    cloud_authority_after: Option<u64>,
    download_url: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotResponse {
    snapshot: Option<SnapshotDescriptor>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareSnapshotUploadRequest {
    snapshot_sequence: u64,
    payload_bytes: u64,
    payload_sha256: String,
    record_count: u32,
    key_id: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PrepareSnapshotUploadResponse {
    object_key: String,
    upload_url: String,
    token: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommitSnapshotRequest {
    snapshot_sequence: u64,
    object_key: String,
    format_version: u16,
    payload_bytes: u64,
    payload_sha256: String,
    record_count: u32,
    key_id: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommitSnapshotResponse {
    committed: bool,
    snapshot_sequence: u64,
}

#[derive(Deserialize)]
struct ReadSnapshotRpcRow {
    snapshot_sequence: i64,
    object_key: String,
    format_version: i16,
    payload_bytes: i64,
    payload_sha256: String,
    record_count: i32,
    key_id: String,
    head_sequence: i64,
    cloud_authority_after: Option<i64>,
}

#[derive(Deserialize)]
struct CommitSnapshotRpcRow {
    committed: bool,
    #[serde(default)]
    retired_object_keys: Vec<String>,
}

pub(super) fn router() -> Router<ReplicaState> {
    Router::new()
        .route("/e2ee/witness/{workspace_id}/snapshot", get(read))
        .route(
            "/e2ee/witness/{workspace_id}/snapshot/upload",
            post(prepare_upload),
        )
        .route("/e2ee/witness/{workspace_id}/snapshot/commit", post(commit))
}

#[utoipa::path(
    get,
    path = "/e2ee/witness/{workspace_id}/snapshot",
    tag = "sync",
    operation_id = "read_latest_e2ee_snapshot",
    params(("workspace_id" = String, Path, description = "Workspace ID")),
    responses(
        (status = 200, description = "Latest encrypted workspace snapshot descriptor", body = SnapshotResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Workspace access denied"),
        (status = 502, description = "Snapshot service unavailable")
    )
)]
pub(super) async fn read(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    Path(workspace_id): Path<String>,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<SnapshotResponse>,
)> {
    require_witness_workspace(&auth, &workspace_id)?;
    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/read_latest_e2ee_snapshot",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(WITNESS_TIMEOUT)
        .json(&serde_json::json!({
            "p_actor_user_id": auth.claims.sub,
            "p_workspace_id": workspace_id,
        }))
        .send()
        .await
        .map_err(|error| witness_transport_error(error, "snapshot read"))?;
    let (status, bytes) = read_bounded_response(response, "snapshot read").await?;
    if is_missing_v2_rpc(status, &bytes) {
        return Ok((
            [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
            Json(SnapshotResponse { snapshot: None }),
        ));
    }
    if !status.is_success() {
        return Err(map_postgrest_error(status, &bytes));
    }
    let mut rows: Vec<ReadSnapshotRpcRow> = serde_json::from_slice(&bytes).map_err(|error| {
        tracing::warn!(%error, "Supabase E2EE snapshot read response was invalid");
        SyncError::E2eeWitnessServiceUnavailable
    })?;
    let Some(row) = rows.pop() else {
        return Ok((
            [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
            Json(SnapshotResponse { snapshot: None }),
        ));
    };

    let snapshot_sequence = u64::try_from(row.snapshot_sequence)
        .map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
    let format_version =
        u16::try_from(row.format_version).map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
    let payload_bytes =
        u64::try_from(row.payload_bytes).map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
    let record_count =
        u32::try_from(row.record_count).map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
    let head_sequence =
        u64::try_from(row.head_sequence).map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
    let cloud_authority_after = row
        .cloud_authority_after
        .map(u64::try_from)
        .transpose()
        .map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
    let download_url = state
        .storage
        .create_signed_url(
            SNAPSHOT_BUCKET,
            &row.object_key,
            SNAPSHOT_DOWNLOAD_URL_TTL_SECONDS,
        )
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Failed to sign E2EE snapshot download URL");
            SyncError::E2eeWitnessServiceUnavailable
        })?;

    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(SnapshotResponse {
            snapshot: Some(SnapshotDescriptor {
                snapshot_sequence,
                format_version,
                payload_bytes,
                payload_sha256: row.payload_sha256,
                record_count,
                key_id: row.key_id,
                head_sequence,
                cloud_authority_after,
                download_url,
            }),
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/e2ee/witness/{workspace_id}/snapshot/upload",
    tag = "sync",
    operation_id = "prepare_e2ee_snapshot_upload",
    params(("workspace_id" = String, Path, description = "Workspace ID")),
    request_body = PrepareSnapshotUploadRequest,
    responses(
        (status = 200, description = "Signed Storage upload URL for encrypted snapshot", body = PrepareSnapshotUploadResponse),
        (status = 400, description = "Invalid snapshot upload metadata"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Workspace access denied"),
        (status = 502, description = "Snapshot service unavailable")
    )
)]
pub(super) async fn prepare_upload(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<PrepareSnapshotUploadRequest>,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<PrepareSnapshotUploadResponse>,
)> {
    require_witness_workspace(&auth, &workspace_id)?;
    validate_snapshot_metadata(
        request.snapshot_sequence,
        request.payload_bytes,
        &request.payload_sha256,
        request.record_count,
        &request.key_id,
    )?;
    let head = read_witness_head(&state, &auth.claims.sub, &workspace_id).await?;
    if !head.initialized || request.snapshot_sequence > head.head_sequence {
        return Err(SyncError::BadRequest(
            "Invalid E2EE snapshot sequence".to_string(),
        ));
    }
    let object_key = format!(
        "{workspace_id}/{}-{}.ans1",
        request.snapshot_sequence,
        Uuid::new_v4()
    );
    let signed = state
        .storage
        .create_signed_upload(SNAPSHOT_BUCKET, &object_key)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Failed to create E2EE snapshot signed upload URL");
            SyncError::E2eeWitnessServiceUnavailable
        })?;
    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(PrepareSnapshotUploadResponse {
            object_key,
            upload_url: signed.signed_url,
            token: signed.token,
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/e2ee/witness/{workspace_id}/snapshot/commit",
    tag = "sync",
    operation_id = "commit_e2ee_snapshot",
    params(("workspace_id" = String, Path, description = "Workspace ID")),
    request_body = CommitSnapshotRequest,
    responses(
        (status = 200, description = "Snapshot metadata committed", body = CommitSnapshotResponse),
        (status = 400, description = "Invalid snapshot commit or checksum mismatch"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Workspace access denied"),
        (status = 502, description = "Snapshot service unavailable")
    )
)]
pub(super) async fn commit(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<CommitSnapshotRequest>,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<CommitSnapshotResponse>,
)> {
    require_witness_workspace(&auth, &workspace_id)?;
    if request.format_version != 1 {
        return Err(SyncError::BadRequest(
            "Unsupported E2EE snapshot format version".to_string(),
        ));
    }
    validate_snapshot_metadata(
        request.snapshot_sequence,
        request.payload_bytes,
        &request.payload_sha256,
        request.record_count,
        &request.key_id,
    )?;
    if !is_valid_snapshot_object_key(&workspace_id, &request.object_key) {
        return Err(SyncError::BadRequest(
            "Invalid E2EE snapshot object key".to_string(),
        ));
    }
    let actual_sha256 = state
        .storage
        .object_sha256(SNAPSHOT_BUCKET, &request.object_key, request.payload_bytes)
        .await
        .map_err(|error| {
            let message = error.to_string();
            if message.contains("did not match") || message.contains("404") {
                SyncError::BadRequest("Uploaded E2EE snapshot size mismatch".to_string())
            } else {
                tracing::warn!(%error, "Failed to verify uploaded E2EE snapshot");
                SyncError::E2eeWitnessServiceUnavailable
            }
        })?;
    if actual_sha256 != request.payload_sha256 {
        return Err(SyncError::BadRequest(
            "Uploaded E2EE snapshot checksum mismatch".to_string(),
        ));
    }
    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/commit_e2ee_snapshot",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(WITNESS_TIMEOUT)
        .json(&serde_json::json!({
            "p_actor_user_id": auth.claims.sub,
            "p_workspace_id": workspace_id,
            "p_snapshot_sequence": request.snapshot_sequence,
            "p_object_key": request.object_key,
            "p_format_version": request.format_version,
            "p_payload_bytes": request.payload_bytes,
            "p_payload_sha256": request.payload_sha256,
            "p_record_count": request.record_count,
            "p_key_id": request.key_id,
        }))
        .send()
        .await
        .map_err(|error| witness_transport_error(error, "snapshot commit"))?;
    let (status, bytes) = read_bounded_response(response, "snapshot commit").await?;
    if !status.is_success() {
        return Err(map_postgrest_error(status, &bytes));
    }
    let mut rows: Vec<CommitSnapshotRpcRow> = serde_json::from_slice(&bytes).map_err(|error| {
        tracing::warn!(%error, "Supabase E2EE snapshot commit response was invalid");
        SyncError::E2eeWitnessServiceUnavailable
    })?;
    let Some(row) = rows.pop() else {
        return Err(SyncError::E2eeWitnessServiceUnavailable);
    };
    if !row.retired_object_keys.is_empty() {
        let _ = state
            .storage
            .delete_files(SNAPSHOT_BUCKET, &row.retired_object_keys)
            .await;
    }

    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(CommitSnapshotResponse {
            committed: row.committed,
            snapshot_sequence: request.snapshot_sequence,
        }),
    ))
}

fn validate_snapshot_metadata(
    snapshot_sequence: u64,
    payload_bytes: u64,
    payload_sha256: &str,
    record_count: u32,
    key_id: &str,
) -> Result<()> {
    if snapshot_sequence == 0
        || payload_bytes == 0
        || payload_bytes > MAX_SNAPSHOT_BYTES
        || i64::try_from(snapshot_sequence).is_err()
        || record_count > 5_000_000
        || !is_sha256_hex(payload_sha256)
        || !is_key_id(key_id)
    {
        return Err(SyncError::BadRequest(
            "Invalid E2EE snapshot metadata".to_string(),
        ));
    }
    Ok(())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn is_key_id(value: &str) -> bool {
    value.len() == 22
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn is_valid_snapshot_object_key(workspace_id: &str, object_key: &str) -> bool {
    let Some(rest) = object_key.strip_prefix(&format!("{workspace_id}/")) else {
        return false;
    };
    let Some(stem) = rest.strip_suffix(".ans1") else {
        return false;
    };
    let Some((seq, uuid_str)) = stem.split_once('-') else {
        return false;
    };
    !seq.is_empty()
        && seq.bytes().all(|byte| byte.is_ascii_digit())
        && Uuid::parse_str(uuid_str).is_ok_and(|parsed| parsed.to_string() == uuid_str)
}

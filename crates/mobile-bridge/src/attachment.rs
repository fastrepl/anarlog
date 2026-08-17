use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use anlg_attachment_sync_core::{
    CacheFileGuard, DownloadObject, MAX_PLAINTEXT_BYTES, TransferPaths, download_to_path,
    ensure_file_size, file_matches_cancellable_async, persist_staged_attachment,
    private_cache_path, read_range, seal_attachment_to_cache, stage_attachment_restore,
    valid_cache_id, valid_sha256, validate_range, validate_signed_download_url,
};
use anlg_e2ee::{
    AttachmentBlobCiphertextMetadata, AttachmentBlobContext, AttachmentBlobMetadata,
    AttachmentBlobPlaintextMetadata,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tokio_util::sync::CancellationToken;
use uuid::{Uuid, Version};

use crate::error::{BridgeError, attachment_error, serialization_error};

const FORMAT_VERSION: u8 = 1;

#[derive(Clone)]
pub(crate) struct TransferOperation {
    cancellation: CancellationToken,
    phase: Arc<Mutex<TransferPhase>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TransferPhase {
    Running,
    Committing,
    Cancelled,
}

impl TransferOperation {
    pub(crate) fn new() -> Self {
        Self {
            cancellation: CancellationToken::new(),
            phase: Arc::new(Mutex::new(TransferPhase::Running)),
        }
    }

    fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    fn ensure_active(&self) -> Result<(), BridgeError> {
        if self.cancellation.is_cancelled() {
            return Err(attachment_error("attachment transfer was cancelled"));
        }
        Ok(())
    }

    fn begin_commit(&self) -> Result<(), BridgeError> {
        let mut phase = self
            .phase
            .lock()
            .map_err(|_| attachment_error("attachment transfer state is unavailable"))?;
        if *phase != TransferPhase::Running || self.cancellation.is_cancelled() {
            return Err(attachment_error("attachment transfer was cancelled"));
        }
        *phase = TransferPhase::Committing;
        Ok(())
    }

    pub(crate) fn cancel(&self) -> bool {
        self.cancellation.cancel();
        let Ok(mut phase) = self.phase.lock() else {
            return true;
        };
        if *phase != TransferPhase::Running {
            return false;
        }
        *phase = TransferPhase::Cancelled;
        true
    }
}

#[derive(Clone)]
pub(crate) struct AttachmentStorage {
    sessions_root: PathBuf,
    transfer_paths: TransferPaths,
}

impl AttachmentStorage {
    pub(crate) fn new(documents_root: String, cache_root: String) -> Result<Self, BridgeError> {
        let documents_root = canonical_directory(documents_root)?;
        let cache_root = canonical_directory(cache_root)?;
        let sessions_root = documents_root.join("sessions");
        std::fs::create_dir_all(&sessions_root).map_err(attachment_error)?;
        if std::fs::canonicalize(&sessions_root).map_err(attachment_error)? != sessions_root {
            return Err(attachment_error("attachment sessions path is invalid"));
        }
        Ok(Self {
            transfer_paths: TransferPaths::new(cache_root, documents_root.clone()),
            sessions_root,
        })
    }

    fn destination(&self, attachment: &AttachmentRecord) -> Result<PathBuf, BridgeError> {
        validate_session_id(&attachment.session_id)?;
        validate_audio_relative_path(&attachment.relative_path)?;
        let session_directory = self.session_directory(&attachment.session_id, true)?;
        Ok(session_directory.join(&attachment.relative_path))
    }

    fn source(&self, attachment: &UploadRecord) -> Result<PathBuf, BridgeError> {
        validate_session_id(&attachment.session_id)?;
        validate_audio_relative_path(&attachment.relative_path)?;
        let session_directory = self.session_directory(&attachment.session_id, false)?;
        let source = session_directory.join(&attachment.relative_path);
        if std::fs::canonicalize(&source).map_err(attachment_error)? != source {
            return Err(attachment_error("attachment source path is invalid"));
        }
        Ok(source)
    }

    fn session_directory(&self, session_id: &str, create: bool) -> Result<PathBuf, BridgeError> {
        let directory = self.sessions_root.join(session_id);
        if create {
            std::fs::create_dir_all(&directory).map_err(attachment_error)?;
        }
        if std::fs::canonicalize(&directory).map_err(attachment_error)? != directory {
            return Err(attachment_error("attachment session path is invalid"));
        }
        Ok(directory)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreAttachmentRequest {
    session_id: String,
    attachment_id: String,
    object_id: String,
    object_key: String,
    signed_url: String,
    supabase_url: String,
    ciphertext_sha256: String,
    ciphertext_size_bytes: u64,
    format_version: u8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RestoredAttachment {
    attachment_id: String,
    session_id: String,
    relative_path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone, FromRow)]
struct AttachmentRecord {
    attachment_id: String,
    session_id: String,
    workspace_id: String,
    relative_path: String,
    source_type: String,
    sha256: String,
    size_bytes: i64,
    cloud_object_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadDescriptor {
    attachment_ref: String,
    version_ref: String,
    ciphertext_size_bytes: u64,
    format_version: u8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreparedUpload {
    cache_id: String,
    ciphertext_sha256: String,
    ciphertext_size_bytes: u64,
}

pub(crate) struct PrepareUploadRequest {
    pub(crate) job_id: String,
    pub(crate) attempt_count: u32,
    pub(crate) object_id: String,
    pub(crate) object_key: String,
}

pub(crate) struct UploadRangeRequest {
    pub(crate) job_id: String,
    pub(crate) attempt_count: u32,
    pub(crate) cache_id: String,
    pub(crate) start: u32,
    pub(crate) end: u32,
}

#[derive(Debug, Clone, FromRow)]
struct UploadRecord {
    job_id: String,
    attachment_id: String,
    session_id: String,
    workspace_id: String,
    expected_sha256: String,
    expected_size_bytes: i64,
    ciphertext_sha256: String,
    ciphertext_size_bytes: i64,
    remote_object_id: String,
    object_key: String,
    cache_id: String,
    phase: String,
    relative_path: String,
    source_type: String,
    attachment_sha256: String,
    attachment_size_bytes: i64,
    cloud_sync_enabled: i64,
}

pub(crate) async fn describe_upload(
    db: Arc<anlg_db_core::Db>,
    e2ee_sync_hook: Arc<anlg_db_sync::E2eeSyncHook>,
    job_id: String,
    attempt_count: u32,
    operation: TransferOperation,
) -> Result<String, BridgeError> {
    operation.ensure_active()?;
    let record = load_upload_record(db.pool(), &job_id, attempt_count).await?;
    let plaintext = validate_upload_record(&record)?;
    let key = workspace_key(&e2ee_sync_hook, &record.workspace_id)?;
    let descriptor = UploadDescriptor {
        attachment_ref: key
            .blind_attachment_backup_ref(&record.workspace_id, &record.attachment_id)
            .map_err(attachment_error)?,
        version_ref: key
            .blind_attachment_backup_version_ref(
                &record.workspace_id,
                &record.attachment_id,
                &plaintext,
            )
            .map_err(attachment_error)?,
        ciphertext_size_bytes: key
            .attachment_blob_ciphertext_size(
                &record.workspace_id,
                &record.attachment_id,
                plaintext.size_bytes,
            )
            .map_err(attachment_error)?,
        format_version: FORMAT_VERSION,
    };
    operation.ensure_active()?;
    serde_json::to_string(&descriptor).map_err(serialization_error)
}

pub(crate) async fn prepare_upload(
    db: Arc<anlg_db_core::Db>,
    e2ee_sync_hook: Arc<anlg_db_sync::E2eeSyncHook>,
    storage: AttachmentStorage,
    request: PrepareUploadRequest,
    operation: TransferOperation,
) -> Result<String, BridgeError> {
    operation.ensure_active()?;
    validate_private_object_identity(&request.object_id, &request.object_key)?;
    let record = load_upload_record(db.pool(), &request.job_id, request.attempt_count).await?;
    let plaintext = validate_upload_record(&record)?;
    let key = workspace_key(&e2ee_sync_hook, &record.workspace_id)?;
    let ciphertext_size_bytes = key
        .attachment_blob_ciphertext_size(
            &record.workspace_id,
            &record.attachment_id,
            plaintext.size_bytes,
        )
        .map_err(attachment_error)?;
    let cache_root = storage.transfer_paths.private_cache_root();
    tokio::fs::create_dir_all(&cache_root)
        .await
        .map_err(attachment_error)?;

    if record.remote_object_id == request.object_id
        && record.object_key == request.object_key
        && matches!(
            record.phase.as_str(),
            "ready" | "transferring" | "finalizing"
        )
        && valid_cache_id(&record.cache_id)
        && valid_sha256(&record.ciphertext_sha256)
        && u64::try_from(record.ciphertext_size_bytes).ok() == Some(ciphertext_size_bytes)
    {
        let cache_path =
            private_cache_path(&cache_root, &record.cache_id).map_err(attachment_error)?;
        if file_matches_cancellable_async(
            cache_path,
            ciphertext_size_bytes,
            record.ciphertext_sha256.clone(),
            operation.cancellation().clone(),
        )
        .await
        .map_err(attachment_error)?
        {
            operation.ensure_active()?;
            return serde_json::to_string(&PreparedUpload {
                cache_id: record.cache_id,
                ciphertext_sha256: record.ciphertext_sha256,
                ciphertext_size_bytes,
            })
            .map_err(serialization_error);
        }
    }

    let source_path = storage.source(&record)?;
    ensure_file_size(&source_path, plaintext.size_bytes).map_err(attachment_error)?;
    let cache_id = Uuid::new_v4().to_string();
    let cache_path = private_cache_path(&cache_root, &cache_id).map_err(attachment_error)?;
    let context = AttachmentBlobContext::new(
        record.workspace_id.clone(),
        record.attachment_id.clone(),
        request.object_id.clone(),
    )
    .map_err(attachment_error)?;
    let (metadata, cache_guard) =
        seal_attachment_to_cache(key, context, source_path, cache_path, plaintext)
            .await
            .map_err(attachment_error)?;
    operation.ensure_active()?;
    let ciphertext_sha256 = metadata.ciphertext.sha256_hex();
    operation.begin_commit()?;
    let updated = sqlx::query(
        "UPDATE attachment_transfer_jobs
         SET remote_object_id = ?, object_key = ?, cache_id = ?,
             ciphertext_sha256 = ?, ciphertext_size_bytes = ?, phase = 'ready',
             last_error = '', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND direction = 'upload' AND phase <> 'completed'
           AND attachment_id = ? AND session_id = ? AND workspace_id = ?
           AND expected_sha256 = ? AND expected_size_bytes = ?
           AND remote_object_id = ? AND object_key = ?
           AND cache_id = ? AND phase = ? AND attempt_count = ?",
    )
    .bind(&request.object_id)
    .bind(&request.object_key)
    .bind(&cache_id)
    .bind(&ciphertext_sha256)
    .bind(i64::try_from(metadata.ciphertext.size_bytes).map_err(attachment_error)?)
    .bind(&record.job_id)
    .bind(&record.attachment_id)
    .bind(&record.session_id)
    .bind(&record.workspace_id)
    .bind(&record.expected_sha256)
    .bind(record.expected_size_bytes)
    .bind(&record.remote_object_id)
    .bind(&record.object_key)
    .bind(&record.cache_id)
    .bind(&record.phase)
    .bind(i64::from(request.attempt_count))
    .execute(db.pool())
    .await
    .map_err(attachment_error)?;
    if updated.rows_affected() != 1 {
        return Err(attachment_error("attachment upload state changed"));
    }
    cache_guard.disarm();

    if record.cache_id != cache_id && valid_cache_id(&record.cache_id) {
        let old_path =
            private_cache_path(&cache_root, &record.cache_id).map_err(attachment_error)?;
        let _ = tokio::fs::remove_file(old_path).await;
    }

    serde_json::to_string(&PreparedUpload {
        cache_id,
        ciphertext_sha256,
        ciphertext_size_bytes: metadata.ciphertext.size_bytes,
    })
    .map_err(serialization_error)
}

pub(crate) async fn read_upload_range(
    db: Arc<anlg_db_core::Db>,
    storage: AttachmentStorage,
    request: UploadRangeRequest,
    operation: TransferOperation,
) -> Result<Vec<u8>, BridgeError> {
    operation.ensure_active()?;
    let start = u64::from(request.start);
    let end = u64::from(request.end);
    validate_range(start, end).map_err(attachment_error)?;
    if !valid_cache_id(&request.cache_id) {
        return Err(attachment_error("attachment upload cache is invalid"));
    }
    let record = load_upload_record(db.pool(), &request.job_id, request.attempt_count).await?;
    validate_upload_record(&record)?;
    if !matches!(
        record.phase.as_str(),
        "ready" | "transferring" | "finalizing"
    ) || record.cache_id != request.cache_id
        || !valid_sha256(&record.ciphertext_sha256)
    {
        return Err(attachment_error("attachment upload state is invalid"));
    }
    let size = u64::try_from(record.ciphertext_size_bytes).map_err(attachment_error)?;
    if size == 0 || end > size {
        return Err(attachment_error("attachment upload range is invalid"));
    }
    let path = private_cache_path(
        &storage.transfer_paths.private_cache_root(),
        &request.cache_id,
    )
    .map_err(attachment_error)?;
    let bytes = read_range(path, start, end, size)
        .await
        .map_err(attachment_error)?;
    operation.ensure_active()?;
    Ok(bytes)
}

pub(crate) async fn cleanup_upload_cache(
    db: Arc<anlg_db_core::Db>,
    storage: AttachmentStorage,
    job_id: String,
    attempt_count: u32,
    cache_id: String,
    operation: TransferOperation,
) -> Result<bool, BridgeError> {
    operation.ensure_active()?;
    validate_opaque_id(&job_id)?;
    if attempt_count == 0 || !valid_cache_id(&cache_id) {
        return Err(attachment_error("attachment upload cache is invalid"));
    }
    let current: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM attachment_transfer_jobs
           WHERE id = ? AND attempt_count = ? AND cache_id = ?
         )",
    )
    .bind(&job_id)
    .bind(i64::from(attempt_count))
    .bind(&cache_id)
    .fetch_one(db.pool())
    .await
    .map_err(attachment_error)?;
    if !current {
        return Ok(false);
    }
    operation.begin_commit()?;
    let path = private_cache_path(&storage.transfer_paths.private_cache_root(), &cache_id)
        .map_err(attachment_error)?;
    let removed = match tokio::fs::remove_file(path).await {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(attachment_error(error)),
    };
    sqlx::query(
        "UPDATE attachment_transfer_jobs
         SET cache_id = '', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND attempt_count = ? AND cache_id = ?",
    )
    .bind(&job_id)
    .bind(i64::from(attempt_count))
    .bind(&cache_id)
    .execute(db.pool())
    .await
    .map_err(attachment_error)?;
    Ok(removed)
}

pub(crate) async fn restore_attachment(
    db: Arc<anlg_db_core::Db>,
    e2ee_sync_hook: Arc<anlg_db_sync::E2eeSyncHook>,
    storage: AttachmentStorage,
    request_json: String,
    operation: TransferOperation,
) -> Result<String, BridgeError> {
    let request: RestoreAttachmentRequest =
        serde_json::from_str(&request_json).map_err(|error| {
            BridgeError::InvalidAttachmentRequestJson {
                reason: error.to_string(),
            }
        })?;
    validate_request(&request)?;
    let attachment = load_attachment(db.pool(), &request).await?;
    validate_attachment(&attachment, &request)?;

    let plaintext_size = u64::try_from(attachment.size_bytes).map_err(attachment_error)?;
    let plaintext = AttachmentBlobPlaintextMetadata::from_hex(plaintext_size, &attachment.sha256)
        .map_err(attachment_error)?;
    let key = e2ee_sync_hook
        .workspace_key(&attachment.workspace_id)
        .ok_or_else(|| attachment_error("attachment workspace key is unavailable"))?;
    if key
        .attachment_blob_ciphertext_size(
            &attachment.workspace_id,
            &attachment.attachment_id,
            plaintext.size_bytes,
        )
        .map_err(attachment_error)?
        != request.ciphertext_size_bytes
    {
        return Err(attachment_error("attachment ciphertext size is invalid"));
    }

    let url = validate_signed_download_url(
        &request.signed_url,
        &request.supabase_url,
        DownloadObject::Private(&request.object_key),
    )
    .map_err(attachment_error)?;
    let cache_root = storage.transfer_paths.private_cache_root();
    tokio::fs::create_dir_all(&cache_root)
        .await
        .map_err(attachment_error)?;
    let cache_id = Uuid::new_v4().to_string();
    let cache_path = private_cache_path(&cache_root, &cache_id).map_err(attachment_error)?;
    let _cache_guard = CacheFileGuard::new(cache_path.clone());
    download_to_path(
        url,
        &cache_path,
        request.ciphertext_size_bytes,
        &request.ciphertext_sha256,
        true,
        operation.cancellation(),
    )
    .await
    .map_err(attachment_error)?;

    let destination = storage.destination(&attachment)?;
    let destination_parent = destination
        .parent()
        .ok_or_else(|| attachment_error("attachment destination is invalid"))?
        .to_path_buf();
    let context = AttachmentBlobContext::new(
        attachment.workspace_id.clone(),
        attachment.attachment_id.clone(),
        request.object_id.clone(),
    )
    .map_err(attachment_error)?;
    let expected = AttachmentBlobMetadata {
        version: request.format_version,
        plaintext,
        ciphertext: AttachmentBlobCiphertextMetadata::from_hex(
            request.ciphertext_size_bytes,
            &request.ciphertext_sha256,
        )
        .map_err(attachment_error)?,
    };
    let cache_path_for_restore = cache_path.clone();
    let staged = tokio::task::spawn_blocking(move || {
        stage_attachment_restore(
            &key,
            &context,
            &cache_path_for_restore,
            &destination_parent,
            &expected,
        )
    })
    .await
    .map_err(attachment_error)?
    .map_err(attachment_error)?;

    let mut transaction = db
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(attachment_error)?;
    let canonical: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM session_attachments
           WHERE id = ? AND session_id = ? AND workspace_id = ?
             AND relative_path = ? AND source_type = ? AND sha256 = ?
             AND size_bytes = ? AND cloud_object_key = ? AND deleted_at IS NULL
         )",
    )
    .bind(&attachment.attachment_id)
    .bind(&attachment.session_id)
    .bind(&attachment.workspace_id)
    .bind(&attachment.relative_path)
    .bind(&attachment.source_type)
    .bind(&attachment.sha256)
    .bind(attachment.size_bytes)
    .bind(&attachment.cloud_object_key)
    .fetch_one(&mut *transaction)
    .await
    .map_err(attachment_error)?;
    if !canonical {
        return Err(attachment_error("attachment changed during restore"));
    }
    operation.begin_commit()?;
    persist_staged_attachment(staged, &destination).map_err(attachment_error)?;
    let local_state = sqlx::query(
        "INSERT INTO attachment_local_state (
           attachment_id, session_id, relative_path, availability, updated_at
         ) VALUES (?, ?, ?, 'present', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(attachment_id) DO UPDATE SET
           session_id = excluded.session_id,
           relative_path = excluded.relative_path,
           availability = excluded.availability,
           updated_at = excluded.updated_at",
    )
    .bind(&attachment.attachment_id)
    .bind(&attachment.session_id)
    .bind(&attachment.relative_path)
    .execute(&mut *transaction)
    .await
    .map_err(attachment_error)?;
    if local_state.rows_affected() != 1 {
        return Err(attachment_error("attachment local state was not updated"));
    }
    transaction.commit().await.map_err(attachment_error)?;

    serde_json::to_string(&RestoredAttachment {
        attachment_id: attachment.attachment_id,
        session_id: attachment.session_id,
        relative_path: attachment.relative_path,
        size_bytes: plaintext_size,
        sha256: attachment.sha256,
    })
    .map_err(serialization_error)
}

async fn load_attachment(
    pool: &sqlx::SqlitePool,
    request: &RestoreAttachmentRequest,
) -> Result<AttachmentRecord, BridgeError> {
    sqlx::query_as(
        "SELECT id AS attachment_id, session_id, workspace_id, relative_path,
                source_type, sha256, size_bytes, cloud_object_key
         FROM session_attachments
         WHERE id = ? AND session_id = ? AND source_type = 'session_audio'
           AND deleted_at IS NULL",
    )
    .bind(&request.attachment_id)
    .bind(&request.session_id)
    .fetch_one(pool)
    .await
    .map_err(attachment_error)
}

async fn load_upload_record(
    pool: &sqlx::SqlitePool,
    job_id: &str,
    attempt_count: u32,
) -> Result<UploadRecord, BridgeError> {
    validate_opaque_id(job_id)?;
    if attempt_count == 0 {
        return Err(attachment_error("attachment upload attempt is invalid"));
    }
    sqlx::query_as(
        "SELECT
           job.id AS job_id,
           job.attachment_id,
           job.session_id,
           job.workspace_id,
           job.expected_sha256,
           job.expected_size_bytes,
           job.ciphertext_sha256,
           job.ciphertext_size_bytes,
           job.remote_object_id,
           job.object_key,
           job.cache_id,
           job.phase,
           attachment.relative_path,
           attachment.source_type,
           attachment.sha256 AS attachment_sha256,
           attachment.size_bytes AS attachment_size_bytes,
           attachment.cloud_sync_enabled
         FROM attachment_transfer_jobs AS job
         JOIN session_attachments AS attachment
           ON attachment.id = job.attachment_id
          AND attachment.session_id = job.session_id
          AND attachment.workspace_id = job.workspace_id
          AND attachment.deleted_at IS NULL
         JOIN attachment_local_state AS local
           ON local.attachment_id = attachment.id
          AND local.availability = 'present'
         WHERE job.id = ? AND job.attempt_count = ?
           AND job.direction = 'upload' AND job.phase <> 'completed'
         LIMIT 1",
    )
    .bind(job_id)
    .bind(i64::from(attempt_count))
    .fetch_one(pool)
    .await
    .map_err(attachment_error)
}

fn validate_upload_record(
    record: &UploadRecord,
) -> Result<AttachmentBlobPlaintextMetadata, BridgeError> {
    if record.job_id.is_empty()
        || record.attachment_id != format!("session-audio:{}", record.session_id)
        || record.workspace_id.is_empty()
        || record.source_type != "session_audio"
        || record.expected_sha256 != record.attachment_sha256
        || record.expected_size_bytes != record.attachment_size_bytes
        || record.cloud_sync_enabled != 1
        || !valid_sha256(&record.expected_sha256)
    {
        return Err(attachment_error("attachment upload state is invalid"));
    }
    validate_session_id(&record.session_id)?;
    validate_audio_relative_path(&record.relative_path)?;
    let size_bytes = u64::try_from(record.expected_size_bytes).map_err(attachment_error)?;
    if size_bytes == 0 || size_bytes > MAX_PLAINTEXT_BYTES {
        return Err(attachment_error("attachment upload size is invalid"));
    }
    AttachmentBlobPlaintextMetadata::from_hex(size_bytes, &record.expected_sha256)
        .map_err(attachment_error)
}

fn workspace_key(
    hook: &anlg_db_sync::E2eeSyncHook,
    workspace_id: &str,
) -> Result<anlg_e2ee::WorkspaceKey, BridgeError> {
    hook.workspace_key(workspace_id)
        .ok_or_else(|| attachment_error("attachment workspace key is unavailable"))
}

fn validate_opaque_id(value: &str) -> Result<(), BridgeError> {
    if value.is_empty()
        || value.len() > 512
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(attachment_error("attachment identifier is invalid"));
    }
    Ok(())
}

fn validate_request(request: &RestoreAttachmentRequest) -> Result<(), BridgeError> {
    validate_session_id(&request.session_id)?;
    if request.attachment_id != format!("session-audio:{}", request.session_id)
        || request.format_version != FORMAT_VERSION
        || !valid_sha256(&request.ciphertext_sha256)
        || request.ciphertext_size_bytes == 0
    {
        return Err(attachment_error("attachment restore metadata is invalid"));
    }
    validate_private_object_identity(&request.object_id, &request.object_key)
}

fn validate_attachment(
    attachment: &AttachmentRecord,
    request: &RestoreAttachmentRequest,
) -> Result<(), BridgeError> {
    if attachment.attachment_id != request.attachment_id
        || attachment.session_id != request.session_id
        || attachment.workspace_id.is_empty()
        || attachment.source_type != "session_audio"
        || !valid_sha256(&attachment.sha256)
        || attachment.size_bytes <= 0
        || attachment.cloud_object_key != request.object_key
    {
        return Err(attachment_error("attachment restore state is invalid"));
    }
    Ok(())
}

fn canonical_directory(value: String) -> Result<PathBuf, BridgeError> {
    let path = PathBuf::from(value);
    if !path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
        return Err(attachment_error("attachment storage path is invalid"));
    }
    std::fs::create_dir_all(&path).map_err(attachment_error)?;
    std::fs::canonicalize(path).map_err(attachment_error)
}

fn validate_session_id(value: &str) -> Result<(), BridgeError> {
    let uuid = Uuid::parse_str(value).map_err(attachment_error)?;
    if uuid.to_string() != value || uuid.get_version() != Some(Version::Random) {
        return Err(attachment_error("attachment session id is invalid"));
    }
    Ok(())
}

fn validate_private_object_identity(object_id: &str, object_key: &str) -> Result<(), BridgeError> {
    let object_uuid = Uuid::parse_str(object_id).map_err(attachment_error)?;
    let (owner, filename) = object_key
        .split_once('/')
        .ok_or_else(|| attachment_error("attachment object key is invalid"))?;
    let owner_uuid = Uuid::parse_str(owner).map_err(attachment_error)?;
    if object_uuid.to_string() != object_id
        || object_uuid.get_version() != Some(Version::Random)
        || owner_uuid.to_string() != owner
        || filename != format!("{object_id}.anb1")
    {
        return Err(attachment_error("attachment object identity is invalid"));
    }
    Ok(())
}

fn validate_audio_relative_path(value: &str) -> Result<(), BridgeError> {
    let mut components = Path::new(value).components();
    let Some(Component::Normal(filename)) = components.next() else {
        return Err(attachment_error("attachment relative path is invalid"));
    };
    if components.next().is_some() {
        return Err(attachment_error("attachment relative path is invalid"));
    }
    let Some(filename) = filename.to_str() else {
        return Err(attachment_error("attachment filename is invalid"));
    };
    let Some(extension) = filename.strip_prefix("audio.") else {
        return Err(attachment_error("attachment filename is invalid"));
    };
    if !matches!(
        extension,
        "aac" | "caf" | "flac" | "m4a" | "mp3" | "mp4" | "ogg" | "wav" | "webm"
    ) {
        return Err(attachment_error("attachment filename is invalid"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_attachment_sync_core::{hex_digest, seal_attachment_to_cache};
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};

    #[test]
    fn rejects_paths_outside_the_session_audio_layout() {
        for path in [
            "../audio.wav",
            "attachments/audio.wav",
            "restored-audio.wav",
        ] {
            assert!(validate_audio_relative_path(path).is_err());
        }
        assert!(validate_audio_relative_path("audio.wav").is_ok());
        assert!(validate_audio_relative_path("audio.m4a").is_ok());
    }

    #[test]
    fn private_object_identity_is_bound_to_its_key() {
        let owner = Uuid::new_v4();
        let object = Uuid::new_v4();
        let key = format!("{owner}/{object}.anb1");

        assert!(validate_private_object_identity(&object.to_string(), &key).is_ok());
        assert!(validate_private_object_identity(&Uuid::new_v4().to_string(), &key).is_err());
    }

    #[test]
    fn cancellation_cannot_interrupt_the_commit_phase() {
        let cancelled = TransferOperation::new();
        assert!(cancelled.cancel());
        assert!(cancelled.begin_commit().is_err());

        let committing = TransferOperation::new();
        committing.begin_commit().unwrap();
        assert!(!committing.cancel());
    }

    #[tokio::test]
    async fn prepares_reads_and_cleans_an_encrypted_audio_upload() {
        let directory = tempfile::tempdir().unwrap();
        let documents = directory.path().join("documents");
        let cache = directory.path().join("cache");
        let db = Arc::new(
            crate::db::open_app_db(&directory.path().join("app.db"), false)
                .await
                .unwrap(),
        );
        let recovery_key = anlg_e2ee::RecoveryKey::generate().unwrap();
        let workspace_id = Uuid::new_v4().to_string();
        let session_id = Uuid::new_v4().to_string();
        let attachment_id = format!("session-audio:{session_id}");
        let job_id = Uuid::new_v4().to_string();
        let owner_id = Uuid::new_v4();
        let object_id = Uuid::new_v4();
        let object_key = format!("{owner_id}/{object_id}.anb1");
        let plaintext = b"mobile upload recording";
        let plaintext_sha256 = hex_digest(Sha256::digest(plaintext).as_slice());
        let source_directory = documents.join("sessions").join(&session_id);
        std::fs::create_dir_all(&source_directory).unwrap();
        std::fs::write(source_directory.join("audio.wav"), plaintext).unwrap();
        std::fs::create_dir_all(&cache).unwrap();

        sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES (?, ?)")
            .bind(&session_id)
            .bind(&workspace_id)
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO session_attachments (
               id, workspace_id, session_id, filename, relative_path,
               content_type, size_bytes, sha256, cloud_object_key,
               source_type, cloud_sync_enabled
             ) VALUES (?, ?, ?, 'audio.wav', 'audio.wav', 'audio/wav', ?, ?, '',
                       'session_audio', 1)",
        )
        .bind(&attachment_id)
        .bind(&workspace_id)
        .bind(&session_id)
        .bind(i64::try_from(plaintext.len()).unwrap())
        .bind(&plaintext_sha256)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO attachment_local_state (
               attachment_id, session_id, relative_path, availability
             ) VALUES (?, ?, 'audio.wav', 'present')",
        )
        .bind(&attachment_id)
        .bind(&session_id)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO attachment_transfer_jobs (
               id, attachment_id, session_id, workspace_id, direction,
               expected_sha256, expected_size_bytes, phase, attempt_count
             ) VALUES (?, ?, ?, ?, 'upload', ?, ?, 'preparing', 1)",
        )
        .bind(&job_id)
        .bind(&attachment_id)
        .bind(&session_id)
        .bind(&workspace_id)
        .bind(&plaintext_sha256)
        .bind(i64::try_from(plaintext.len()).unwrap())
        .execute(db.pool())
        .await
        .unwrap();

        let hook = Arc::new(anlg_db_sync::E2eeSyncHook::default());
        hook.set_personal_workspace(&workspace_id, &recovery_key)
            .unwrap();
        let storage = AttachmentStorage::new(
            documents.to_string_lossy().into_owned(),
            cache.to_string_lossy().into_owned(),
        )
        .unwrap();
        let descriptor: serde_json::Value = serde_json::from_str(
            &describe_upload(
                Arc::clone(&db),
                Arc::clone(&hook),
                job_id.clone(),
                1,
                TransferOperation::new(),
            )
            .await
            .unwrap(),
        )
        .unwrap();
        assert_eq!(descriptor["formatVersion"], FORMAT_VERSION);
        assert!(descriptor["attachmentRef"].as_str().unwrap().len() >= 32);
        assert!(descriptor["versionRef"].as_str().unwrap().len() >= 32);

        let prepared: serde_json::Value = serde_json::from_str(
            &prepare_upload(
                Arc::clone(&db),
                hook,
                storage.clone(),
                PrepareUploadRequest {
                    job_id: job_id.clone(),
                    attempt_count: 1,
                    object_id: object_id.to_string(),
                    object_key,
                },
                TransferOperation::new(),
            )
            .await
            .unwrap(),
        )
        .unwrap();
        let cache_id = prepared["cacheId"].as_str().unwrap().to_string();
        let ciphertext_size = prepared["ciphertextSizeBytes"].as_u64().unwrap();
        assert_eq!(descriptor["ciphertextSizeBytes"], ciphertext_size);
        let ciphertext = read_upload_range(
            Arc::clone(&db),
            storage.clone(),
            UploadRangeRequest {
                job_id: job_id.clone(),
                attempt_count: 1,
                cache_id: cache_id.clone(),
                start: 0,
                end: u32::try_from(ciphertext_size).unwrap(),
            },
            TransferOperation::new(),
        )
        .await
        .unwrap();
        let key = recovery_key.workspace_key(&workspace_id).unwrap();
        let context =
            AttachmentBlobContext::new(workspace_id, attachment_id, object_id.to_string()).unwrap();
        let expected = AttachmentBlobMetadata {
            version: FORMAT_VERSION,
            plaintext: AttachmentBlobPlaintextMetadata::from_hex(
                plaintext.len() as u64,
                &plaintext_sha256,
            )
            .unwrap(),
            ciphertext: AttachmentBlobCiphertextMetadata::from_hex(
                ciphertext_size,
                prepared["ciphertextSha256"].as_str().unwrap(),
            )
            .unwrap(),
        };
        let mut restored = Vec::new();
        key.open_attachment_blob(
            &context,
            &mut std::io::Cursor::new(ciphertext),
            &mut restored,
            &expected,
        )
        .unwrap();
        assert_eq!(restored, plaintext);

        assert!(
            cleanup_upload_cache(
                Arc::clone(&db),
                storage.clone(),
                job_id.clone(),
                1,
                cache_id.clone(),
                TransferOperation::new(),
            )
            .await
            .unwrap()
        );
        assert!(
            !private_cache_path(&storage.transfer_paths.private_cache_root(), &cache_id)
                .unwrap()
                .exists()
        );
        let current_cache_id: String =
            sqlx::query_scalar("SELECT cache_id FROM attachment_transfer_jobs WHERE id = ?")
                .bind(job_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(current_cache_id.is_empty());
    }

    #[tokio::test]
    async fn restores_encrypted_audio_and_commits_local_state() {
        let directory = tempfile::tempdir().unwrap();
        let documents = directory.path().join("documents");
        let cache = directory.path().join("cache");
        std::fs::create_dir_all(&documents).unwrap();
        std::fs::create_dir_all(&cache).unwrap();
        let db = Arc::new(
            crate::db::open_app_db(&directory.path().join("app.db"), false)
                .await
                .unwrap(),
        );
        let recovery_key = anlg_e2ee::RecoveryKey::generate().unwrap();
        let workspace_id = Uuid::new_v4().to_string();
        let session_id = Uuid::new_v4().to_string();
        let attachment_id = format!("session-audio:{session_id}");
        let owner_id = Uuid::new_v4();
        let object_id = Uuid::new_v4();
        let object_key = format!("{owner_id}/{object_id}.anb1");
        let plaintext = b"mobile encrypted recording";
        let plaintext_sha256 = hex_digest(Sha256::digest(plaintext).as_slice());
        let source = directory.path().join("source.wav");
        let ciphertext = directory.path().join("ciphertext.anb1");
        std::fs::write(&source, plaintext).unwrap();
        let workspace_key = recovery_key.workspace_key(&workspace_id).unwrap();
        let context = AttachmentBlobContext::new(
            workspace_id.clone(),
            attachment_id.clone(),
            object_id.to_string(),
        )
        .unwrap();
        let expected_plaintext =
            AttachmentBlobPlaintextMetadata::from_hex(plaintext.len() as u64, &plaintext_sha256)
                .unwrap();
        let (blob, guard) = seal_attachment_to_cache(
            workspace_key,
            context,
            source,
            ciphertext.clone(),
            expected_plaintext,
        )
        .await
        .unwrap();
        guard.disarm();

        sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES (?, ?)")
            .bind(&session_id)
            .bind(&workspace_id)
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO session_attachments (
               id, workspace_id, session_id, filename, relative_path,
               content_type, size_bytes, sha256, cloud_object_key, source_type
             ) VALUES (?, ?, ?, 'audio.wav', 'audio.wav', 'audio/wav', ?, ?, ?, 'session_audio')",
        )
        .bind(&attachment_id)
        .bind(&workspace_id)
        .bind(&session_id)
        .bind(i64::try_from(plaintext.len()).unwrap())
        .bind(&plaintext_sha256)
        .bind(&object_key)
        .execute(db.pool())
        .await
        .unwrap();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let body = std::fs::read(ciphertext).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        });

        let hook = Arc::new(anlg_db_sync::E2eeSyncHook::default());
        hook.set_personal_workspace(&workspace_id, &recovery_key)
            .unwrap();
        let request = serde_json::json!({
            "sessionId": session_id,
            "attachmentId": attachment_id,
            "objectId": object_id.to_string(),
            "objectKey": object_key,
            "signedUrl": format!(
                "http://{address}/storage/v1/object/sign/attachment-backups/{object_key}?token=test"
            ),
            "supabaseUrl": format!("http://{address}"),
            "ciphertextSha256": blob.ciphertext.sha256_hex(),
            "ciphertextSizeBytes": blob.ciphertext.size_bytes,
            "formatVersion": blob.version,
        });
        let storage = AttachmentStorage::new(
            documents.to_string_lossy().into_owned(),
            cache.to_string_lossy().into_owned(),
        )
        .unwrap();

        let restored: serde_json::Value = serde_json::from_str(
            &restore_attachment(
                Arc::clone(&db),
                hook,
                storage,
                request.to_string(),
                TransferOperation::new(),
            )
            .await
            .unwrap(),
        )
        .unwrap();
        server.join().unwrap();

        assert_eq!(restored["attachmentId"], attachment_id);
        assert_eq!(
            std::fs::read(
                documents
                    .join("sessions")
                    .join(&session_id)
                    .join("audio.wav")
            )
            .unwrap(),
            plaintext
        );
        let local_state: (String, String) = sqlx::query_as(
            "SELECT availability, relative_path FROM attachment_local_state WHERE attachment_id = ?",
        )
        .bind(&attachment_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            local_state,
            ("present".to_string(), "audio.wav".to_string())
        );
    }
}

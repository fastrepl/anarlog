use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

use futures_util::StreamExt;
use hypr_e2ee::{
    AttachmentBlobCiphertextMetadata, AttachmentBlobContext, AttachmentBlobMetadata,
    AttachmentBlobPlaintextMetadata, WorkspaceKey,
};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use tauri::{Manager, Runtime};
use tauri_plugin_settings::SettingsPluginExt;
use tokio::io::AsyncWriteExt;
use uuid::{Uuid, Version};

use crate::control::DownloadOperation;
use crate::error::{Error, Result};
use crate::models::{
    PreparedDeleteGuard, PreparedSharedUpload, PreparedUpload, RestoredAttachment,
    SharedAttachmentCacheResult, UploadDescriptor,
};

const FORMAT_VERSION: i16 = 1;
const MAX_RANGE_BYTES: u64 = 6 * 1024 * 1024;
const MAX_PLAINTEXT_BYTES: u64 = hypr_e2ee::ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES;
const MAX_CIPHERTEXT_BYTES: u64 = 545_259_520;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const DELETE_GUARD_ORPHAN_GRACE: Duration = Duration::from_secs(15 * 60);
const DELETE_PREFLIGHT_SELECT: &str = "SELECT
   job.attachment_id,
   job.session_id,
   job.workspace_id,
   job.expected_sha256,
   job.expected_size_bytes,
   job.object_key,
   job.cache_id,
   job.ciphertext_sha256,
   job.ciphertext_size_bytes,
   attachment.id AS current_attachment_id,
   attachment.relative_path,
   attachment.source_type,
   attachment.sha256 AS attachment_sha256,
   attachment.size_bytes AS attachment_size_bytes,
   attachment.cloud_object_key AS attachment_cloud_object_key,
   attachment.cloud_sync_enabled,
   attachment.deleted_at
 FROM attachment_transfer_jobs AS job
 LEFT JOIN session_attachments AS attachment
   ON attachment.id = job.attachment_id
  AND attachment.session_id = job.session_id
  AND attachment.workspace_id = job.workspace_id
 WHERE job.id = ? AND job.attempt_count = ?
   AND job.direction = 'delete' AND job.phase = 'finalizing'
 LIMIT 1";

#[derive(Debug, Clone, FromRow)]
struct TransferAttachment {
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
    attachment_cloud_object_key: String,
    cloud_sync_enabled: i64,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
struct LocalAttachment {
    attachment_id: String,
    session_id: String,
    workspace_id: String,
    relative_path: String,
    source_type: String,
    sha256: String,
    size_bytes: i64,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
struct SharedUploadAttachment {
    attachment_id: String,
    session_id: String,
    workspace_id: String,
    relative_path: String,
    source_type: String,
    sha256: String,
    size_bytes: i64,
    filename: String,
    content_type: String,
    cloud_sync_enabled: i64,
    cloud_object_key: String,
}

#[derive(Debug, Clone, FromRow)]
struct DeleteSourcePreflight {
    attachment_id: String,
    session_id: String,
    workspace_id: String,
    expected_sha256: String,
    expected_size_bytes: i64,
    object_key: String,
    cache_id: String,
    ciphertext_sha256: String,
    ciphertext_size_bytes: i64,
    current_attachment_id: Option<String>,
    relative_path: Option<String>,
    source_type: Option<String>,
    attachment_sha256: Option<String>,
    attachment_size_bytes: Option<i64>,
    attachment_cloud_object_key: Option<String>,
    cloud_sync_enabled: Option<i64>,
    deleted_at: Option<String>,
}

struct CacheFileGuard {
    path: Option<PathBuf>,
}

impl CacheFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn disarm(mut self) {
        self.path = None;
    }
}

impl Drop for CacheFileGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

struct SharedUploadCacheFileGuard {
    path: Option<PathBuf>,
}

struct DeleteGuardFileGuard {
    path: Option<PathBuf>,
}

struct CancellableReader<R> {
    inner: R,
    cancellation: tokio_util::sync::CancellationToken,
}

impl<R: Read> Read for CancellableReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.cancellation.is_cancelled() {
            return Err(std::io::ErrorKind::ConnectionAborted.into());
        }
        let read = self.inner.read(buffer)?;
        if self.cancellation.is_cancelled() {
            return Err(std::io::ErrorKind::ConnectionAborted.into());
        }
        Ok(read)
    }
}

impl DeleteGuardFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn disarm(mut self) {
        self.path = None;
    }
}

impl Drop for DeleteGuardFileGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = cleanup_delete_guard_path(&path);
        }
    }
}

impl SharedUploadCacheFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn disarm(mut self) {
        self.path = None;
    }
}

impl Drop for SharedUploadCacheFileGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = cleanup_shared_upload_path(&path);
        }
    }
}

pub async fn describe_upload(
    state: &tauri_plugin_db::ManagedState,
    job_id: &str,
    attempt_count: i64,
) -> Result<UploadDescriptor> {
    let record =
        load_transfer_attachment(state.pool(), job_id, attempt_count, "upload", true).await?;
    validate_upload_transfer_version(&record)?;
    let plaintext = plaintext_metadata(&record.expected_sha256, record.expected_size_bytes)?;
    let key = workspace_key(state, &record.workspace_id)?;

    Ok(UploadDescriptor {
        attachment_ref: key
            .blind_attachment_backup_ref(&record.workspace_id, &record.attachment_id)?,
        version_ref: key.blind_attachment_backup_version_ref(
            &record.workspace_id,
            &record.attachment_id,
            &plaintext,
        )?,
        ciphertext_size_bytes: key.attachment_blob_ciphertext_size(
            &record.workspace_id,
            &record.attachment_id,
            plaintext.size_bytes,
        )?,
        format_version: FORMAT_VERSION,
    })
}

pub async fn prepare_upload<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri_plugin_db::ManagedState,
    job_id: &str,
    attempt_count: i64,
    object_id: &str,
    object_key: &str,
) -> Result<PreparedUpload> {
    validate_object_identity(object_id, object_key)?;
    let record =
        load_transfer_attachment(state.pool(), job_id, attempt_count, "upload", true).await?;
    validate_upload_transfer_version(&record)?;
    let expected = plaintext_metadata(&record.expected_sha256, record.expected_size_bytes)?;
    let key = workspace_key(state, &record.workspace_id)?;
    let expected_ciphertext_size = key.attachment_blob_ciphertext_size(
        &record.workspace_id,
        &record.attachment_id,
        expected.size_bytes,
    )?;
    let cache_root = private_cache_root(app)?;
    tokio::fs::create_dir_all(&cache_root).await?;

    if record.remote_object_id == object_id
        && record.object_key == object_key
        && matches!(
            record.phase.as_str(),
            "ready" | "transferring" | "finalizing"
        )
        && valid_cache_id(&record.cache_id)
        && valid_sha256(&record.ciphertext_sha256)
        && u64::try_from(record.ciphertext_size_bytes).ok() == Some(expected_ciphertext_size)
    {
        let existing_path = private_cache_path(&cache_root, &record.cache_id)?;
        if file_matches_async(
            existing_path,
            expected_ciphertext_size,
            record.ciphertext_sha256.clone(),
        )
        .await?
        {
            return Ok(PreparedUpload {
                cache_id: record.cache_id,
                ciphertext_sha256: record.ciphertext_sha256,
                ciphertext_size_bytes: expected_ciphertext_size,
            });
        }
    }

    let source_path = resolve_attachment_path(app, &record.local_attachment(), true)?;
    ensure_file_size(&source_path, expected.size_bytes)?;
    let cache_id = Uuid::new_v4().to_string();
    let cache_path = private_cache_path(&cache_root, &cache_id)?;
    let context = AttachmentBlobContext::new(
        record.workspace_id.clone(),
        record.attachment_id.clone(),
        object_id.to_string(),
    )?;
    let source_path_for_seal = source_path.clone();
    let cache_path_for_seal = cache_path.clone();
    let metadata = tokio::task::spawn_blocking(move || {
        let mut source = std::fs::File::open(source_path_for_seal)?;
        let mut destination = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&cache_path_for_seal)?;
        let cache_guard = CacheFileGuard::new(cache_path_for_seal);
        let metadata =
            key.seal_attachment_blob(&context, &mut source, &mut destination, &expected)?;
        destination.sync_all()?;
        Ok::<_, Error>((metadata, cache_guard))
    })
    .await
    .map_err(|_| Error::CacheUnavailable)?;
    let (metadata, cache_guard) = match metadata {
        Ok(result) => result,
        Err(error) => return Err(error),
    };

    let ciphertext_sha256 = metadata.ciphertext.sha256_hex();
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
    .bind(object_id)
    .bind(object_key)
    .bind(&cache_id)
    .bind(&ciphertext_sha256)
    .bind(i64::try_from(metadata.ciphertext.size_bytes).map_err(|_| Error::InvalidMetadata)?)
    .bind(job_id)
    .bind(&record.attachment_id)
    .bind(&record.session_id)
    .bind(&record.workspace_id)
    .bind(&record.expected_sha256)
    .bind(record.expected_size_bytes)
    .bind(&record.remote_object_id)
    .bind(&record.object_key)
    .bind(&record.cache_id)
    .bind(&record.phase)
    .bind(attempt_count)
    .execute(state.pool())
    .await;
    let updated = match updated {
        Ok(updated) => updated,
        Err(error) => return Err(error.into()),
    };
    if updated.rows_affected() != 1 {
        return Err(Error::InvalidTransferState);
    }

    cache_guard.disarm();

    if record.cache_id != cache_id && valid_cache_id(&record.cache_id) {
        let old_path = private_cache_path(&cache_root, &record.cache_id)?;
        let _ = tokio::fs::remove_file(old_path).await;
    }

    Ok(PreparedUpload {
        cache_id,
        ciphertext_sha256,
        ciphertext_size_bytes: metadata.ciphertext.size_bytes,
    })
}

pub async fn read_upload_range<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri_plugin_db::ManagedState,
    job_id: &str,
    attempt_count: i64,
    cache_id: &str,
    start: u64,
    end: u64,
) -> Result<Vec<u8>> {
    validate_range(start, end)?;
    if !valid_cache_id(cache_id) {
        return Err(Error::InvalidTransferState);
    }
    let record =
        load_transfer_attachment(state.pool(), job_id, attempt_count, "upload", false).await?;
    validate_upload_transfer_version(&record)?;
    if !matches!(
        record.phase.as_str(),
        "ready" | "transferring" | "finalizing"
    ) || record.cache_id != cache_id
        || !valid_cache_id(&record.cache_id)
        || !valid_sha256(&record.ciphertext_sha256)
    {
        return Err(Error::InvalidTransferState);
    }
    let size = u64::try_from(record.ciphertext_size_bytes).map_err(|_| Error::InvalidMetadata)?;
    if size == 0 || size > MAX_CIPHERTEXT_BYTES || end > size {
        return Err(Error::InvalidRange);
    }
    let path = private_cache_path(&private_cache_root(app)?, &record.cache_id)?;
    read_range(path, start, end, size).await
}

#[allow(clippy::too_many_arguments)]
pub async fn prepare_shared_upload<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri_plugin_db::ManagedState,
    operation: &DownloadOperation,
    attachment_id: &str,
    expected_sha256: &str,
    expected_size_bytes: u64,
    expected_filename: &str,
    expected_content_type: &str,
    expected_cloud_object_key: &str,
) -> Result<PreparedSharedUpload> {
    operation.ensure_active()?;
    let attachment = load_shared_upload_attachment(state.pool(), attachment_id).await?;
    validate_shared_upload_version(
        &attachment,
        expected_sha256,
        expected_size_bytes,
        expected_filename,
        expected_content_type,
        expected_cloud_object_key,
    )?;
    let source_path = resolve_attachment_path(app, &attachment.local_attachment(), true)?;
    let cache_root = shared_upload_cache_root(app)?;
    create_shared_upload_cache_root(&cache_root).await?;
    let cache_id = Uuid::new_v4().to_string();
    let cache_path = shared_upload_cache_path(&cache_root, &cache_id)?;
    let source_path_for_snapshot = source_path.clone();
    let cache_path_for_snapshot = cache_path.clone();
    let expected_sha256_for_snapshot = expected_sha256.to_string();
    let cancellation = operation.cancellation().clone();
    let snapshot = tokio::task::spawn_blocking(move || {
        snapshot_verified_file(
            &source_path_for_snapshot,
            &cache_path_for_snapshot,
            expected_size_bytes,
            &expected_sha256_for_snapshot,
            &cancellation,
        )
    })
    .await
    .map_err(|_| Error::CacheUnavailable)?;
    let cache_guard = match snapshot {
        Ok(cache_guard) => cache_guard,
        Err(error) => return Err(error),
    };

    operation.ensure_active()?;
    let current = load_shared_upload_attachment(state.pool(), attachment_id).await?;
    validate_shared_upload_version(
        &current,
        expected_sha256,
        expected_size_bytes,
        expected_filename,
        expected_content_type,
        expected_cloud_object_key,
    )?;
    if current != attachment {
        return Err(Error::InvalidTransferState);
    }

    operation.begin_commit()?;
    cache_guard.disarm();
    Ok(PreparedSharedUpload {
        cache_id,
        sha256: expected_sha256.to_string(),
        size_bytes: expected_size_bytes,
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn read_shared_upload_range<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri_plugin_db::ManagedState,
    attachment_id: &str,
    cache_id: &str,
    expected_sha256: &str,
    expected_size_bytes: u64,
    expected_filename: &str,
    expected_content_type: &str,
    expected_cloud_object_key: &str,
    start: u64,
    end: u64,
) -> Result<Vec<u8>> {
    validate_range(start, end)?;
    let attachment = load_shared_upload_attachment(state.pool(), attachment_id).await?;
    validate_shared_upload_version(
        &attachment,
        expected_sha256,
        expected_size_bytes,
        expected_filename,
        expected_content_type,
        expected_cloud_object_key,
    )?;
    if end > expected_size_bytes {
        return Err(Error::InvalidRange);
    }
    let path = shared_upload_cache_path(&shared_upload_cache_root(app)?, cache_id)?;
    read_range(path, start, end, expected_size_bytes).await
}

#[allow(clippy::too_many_arguments)]
pub async fn validate_shared_upload<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri_plugin_db::ManagedState,
    operation: &DownloadOperation,
    attachment_id: &str,
    cache_id: &str,
    expected_sha256: &str,
    expected_size_bytes: u64,
    expected_filename: &str,
    expected_content_type: &str,
    expected_cloud_object_key: &str,
) -> Result<bool> {
    operation.ensure_active()?;
    let attachment = match load_shared_upload_attachment(state.pool(), attachment_id).await {
        Ok(attachment) => attachment,
        Err(Error::LocalAttachmentUnavailable) => return Ok(false),
        Err(error) => return Err(error),
    };
    if validate_shared_upload_version(
        &attachment,
        expected_sha256,
        expected_size_bytes,
        expected_filename,
        expected_content_type,
        expected_cloud_object_key,
    )
    .is_err()
    {
        return Ok(false);
    }
    let (_, source_path) = attachment_paths(app, &attachment.local_attachment())?;
    let cache_path = shared_upload_cache_path(&shared_upload_cache_root(app)?, cache_id)?;
    let cancellation = operation.cancellation().clone();
    let source_matches = file_matches_cancellable_async(
        source_path,
        expected_size_bytes,
        expected_sha256.to_string(),
        cancellation.clone(),
    );
    let cache_matches = file_matches_cancellable_async(
        cache_path,
        expected_size_bytes,
        expected_sha256.to_string(),
        cancellation,
    );
    let (source_matches, cache_matches) = tokio::join!(source_matches, cache_matches);
    let source_matches = source_matches?;
    let cache_matches = cache_matches?;
    if !source_matches || !cache_matches {
        return Ok(false);
    }
    operation.ensure_active()?;
    let current = match load_shared_upload_attachment(state.pool(), attachment_id).await {
        Ok(attachment) => attachment,
        Err(Error::LocalAttachmentUnavailable) => return Ok(false),
        Err(error) => return Err(error),
    };
    if validate_shared_upload_version(
        &current,
        expected_sha256,
        expected_size_bytes,
        expected_filename,
        expected_content_type,
        expected_cloud_object_key,
    )
    .is_err()
    {
        return Ok(false);
    }
    operation.ensure_active()?;
    Ok(current == attachment)
}

pub async fn cleanup_shared_upload<R: Runtime>(
    app: &tauri::AppHandle<R>,
    cache_id: &str,
) -> Result<bool> {
    let path = shared_upload_cache_path(&shared_upload_cache_root(app)?, cache_id)?;
    tokio::task::spawn_blocking(move || cleanup_shared_upload_path(&path))
        .await
        .map_err(|_| Error::CacheUnavailable)?
}

pub async fn prepare_delete_guard<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri_plugin_db::ManagedState,
    operation: &DownloadOperation,
    job_id: &str,
    attempt_count: i64,
) -> Result<PreparedDeleteGuard> {
    operation.ensure_active()?;
    validate_opaque_id(job_id)?;
    if attempt_count <= 0 {
        return Err(Error::InvalidTransferState);
    }
    let record = sqlx::query_as::<_, DeleteSourcePreflight>(DELETE_PREFLIGHT_SELECT)
        .bind(job_id)
        .bind(attempt_count)
        .fetch_optional(state.pool())
        .await?
        .ok_or(Error::InvalidTransferState)?;
    let expected_size = valid_plaintext_size(record.expected_size_bytes)?;
    if !valid_sha256(&record.expected_sha256) || record.object_key.is_empty() {
        return Err(Error::InvalidMetadata);
    }
    let Some(attachment) = delete_source_attachment(&record)? else {
        clear_delete_guard_link(state.pool(), operation, job_id, attempt_count, &record).await?;
        cleanup_linked_delete_guard(app, &record.cache_id).await;
        return Ok(PreparedDeleteGuard {
            should_delete: true,
            guard_id: String::new(),
        });
    };

    let object_id = private_object_id(&record.object_key)?;
    let key = workspace_key(state, &record.workspace_id)?;
    let expected = plaintext_metadata(&record.expected_sha256, record.expected_size_bytes)?;
    let context = AttachmentBlobContext::new(
        record.workspace_id.clone(),
        record.attachment_id.clone(),
        object_id,
    )?;
    let guard_root = delete_guard_root(app)?;
    create_delete_guard_root(&guard_root).await?;

    if let Some(metadata) = delete_guard_metadata(&record, &key, &expected)? {
        let guard_path = delete_guard_path(&guard_root, &record.cache_id)?;
        if guarded_file_matches_async(
            guard_path,
            metadata.ciphertext.size_bytes,
            metadata.ciphertext.sha256_hex(),
            operation.cancellation().clone(),
        )
        .await?
        {
            operation.ensure_active()?;
            return Ok(PreparedDeleteGuard {
                should_delete: true,
                guard_id: record.cache_id,
            });
        }
    }

    let source_path = match resolve_attachment_path(app, &attachment, true) {
        Ok(path) => path,
        Err(Error::LocalAttachmentUnavailable) => {
            clear_delete_guard_link(state.pool(), operation, job_id, attempt_count, &record)
                .await?;
            cleanup_linked_delete_guard(app, &record.cache_id).await;
            return Ok(PreparedDeleteGuard {
                should_delete: false,
                guard_id: String::new(),
            });
        }
        Err(error) => return Err(error),
    };
    if !file_matches_cancellable_async(
        source_path.clone(),
        expected_size,
        record.expected_sha256.clone(),
        operation.cancellation().clone(),
    )
    .await?
    {
        clear_delete_guard_link(state.pool(), operation, job_id, attempt_count, &record).await?;
        cleanup_linked_delete_guard(app, &record.cache_id).await;
        return Ok(PreparedDeleteGuard {
            should_delete: false,
            guard_id: String::new(),
        });
    }
    operation.ensure_active()?;

    let guard_id = Uuid::new_v4().to_string();
    let guard_path = delete_guard_path(&guard_root, &guard_id)?;
    let guard_path_for_seal = guard_path.clone();
    let operation_cancellation = operation.cancellation().clone();
    let seal_result = tokio::task::spawn_blocking(move || {
        seal_delete_guard(
            &key,
            &context,
            &source_path,
            &guard_path_for_seal,
            &expected,
            &operation_cancellation,
        )
    })
    .await
    .map_err(|_| Error::CacheUnavailable)?;
    let (metadata, guard) = match seal_result {
        Ok(result) => result,
        Err(error) if delete_source_changed(&error) => {
            clear_delete_guard_link(state.pool(), operation, job_id, attempt_count, &record)
                .await?;
            cleanup_linked_delete_guard(app, &record.cache_id).await;
            return Ok(PreparedDeleteGuard {
                should_delete: false,
                guard_id: String::new(),
            });
        }
        Err(error) => return Err(error),
    };
    operation.ensure_active()?;
    let guard_root_for_sync = guard_root.clone();
    tokio::task::spawn_blocking(move || sync_destination_directory(&guard_root_for_sync))
        .await
        .map_err(|_| Error::CacheUnavailable)??;
    operation.ensure_active()?;
    let ciphertext_sha256 = metadata.ciphertext.sha256_hex();
    let ciphertext_size_bytes =
        i64::try_from(metadata.ciphertext.size_bytes).map_err(|_| Error::InvalidMetadata)?;
    operation.begin_commit()?;
    let updated = sqlx::query(
        "UPDATE attachment_transfer_jobs
         SET cache_id = ?, ciphertext_sha256 = ?, ciphertext_size_bytes = ?,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND attempt_count = ?
           AND direction = 'delete' AND phase = 'finalizing'
           AND attachment_id = ? AND session_id = ? AND workspace_id = ?
           AND expected_sha256 = ? AND expected_size_bytes = ? AND object_key = ?
           AND cache_id = ? AND ciphertext_sha256 = ? AND ciphertext_size_bytes = ?
           AND EXISTS (
             SELECT 1 FROM session_attachments AS attachment
             WHERE attachment.id = attachment_transfer_jobs.attachment_id
               AND attachment.session_id = attachment_transfer_jobs.session_id
               AND attachment.workspace_id = attachment_transfer_jobs.workspace_id
               AND attachment.sha256 = attachment_transfer_jobs.expected_sha256
               AND attachment.size_bytes = attachment_transfer_jobs.expected_size_bytes
               AND attachment.cloud_object_key = attachment_transfer_jobs.object_key
               AND attachment.relative_path = ? AND attachment.source_type = ?
               AND attachment.deleted_at IS NULL
           )",
    )
    .bind(&guard_id)
    .bind(&ciphertext_sha256)
    .bind(ciphertext_size_bytes)
    .bind(job_id)
    .bind(attempt_count)
    .bind(&record.attachment_id)
    .bind(&record.session_id)
    .bind(&record.workspace_id)
    .bind(&record.expected_sha256)
    .bind(record.expected_size_bytes)
    .bind(&record.object_key)
    .bind(&record.cache_id)
    .bind(&record.ciphertext_sha256)
    .bind(record.ciphertext_size_bytes)
    .bind(&attachment.relative_path)
    .bind(&attachment.source_type)
    .execute(state.pool())
    .await?;
    if updated.rows_affected() != 1 {
        return Err(Error::DeleteGuardChanged);
    }
    guard.disarm();
    cleanup_linked_delete_guard(app, &record.cache_id).await;
    Ok(PreparedDeleteGuard {
        should_delete: true,
        guard_id,
    })
}

pub async fn commit_delete_guard<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri_plugin_db::ManagedState,
    operation: &DownloadOperation,
    job_id: &str,
    attempt_count: i64,
    guard_id: &str,
) -> Result<()> {
    operation.ensure_active()?;
    validate_opaque_id(job_id)?;
    if attempt_count <= 0 || (!guard_id.is_empty() && !valid_cache_id(guard_id)) {
        return Err(Error::InvalidTransferState);
    }
    let record = sqlx::query_as::<_, DeleteSourcePreflight>(DELETE_PREFLIGHT_SELECT)
        .bind(job_id)
        .bind(attempt_count)
        .fetch_optional(state.pool())
        .await?
        .ok_or(Error::DeleteGuardChanged)?;
    if record.cache_id != guard_id {
        return Err(Error::DeleteGuardChanged);
    }
    if !valid_sha256(&record.expected_sha256) || record.object_key.is_empty() {
        return Err(Error::InvalidTransferState);
    }
    let initial_attachment = delete_source_attachment(&record)?;
    let staged = if let Some(attachment) = initial_attachment.as_ref() {
        if guard_id.is_empty() {
            return Err(Error::DeleteGuardChanged);
        }
        let expected = plaintext_metadata(&record.expected_sha256, record.expected_size_bytes)?;
        let key = workspace_key(state, &record.workspace_id)?;
        let metadata =
            delete_guard_metadata(&record, &key, &expected)?.ok_or(Error::DeleteGuardChanged)?;
        let context = AttachmentBlobContext::new(
            record.workspace_id.clone(),
            record.attachment_id.clone(),
            private_object_id(&record.object_key)?,
        )?;
        let guard_path = delete_guard_path(&delete_guard_root(app)?, guard_id)?;
        if !guarded_file_matches_async(
            guard_path.clone(),
            metadata.ciphertext.size_bytes,
            metadata.ciphertext.sha256_hex(),
            operation.cancellation().clone(),
        )
        .await?
        {
            return Err(Error::CacheUnavailable);
        }
        let (_, destination) = attachment_paths(app, attachment).map_err(|error| match error {
            Error::InvalidMetadata | Error::LocalAttachmentUnavailable => Error::DeleteGuardChanged,
            error => error,
        })?;
        let destination_parent = destination
            .parent()
            .ok_or(Error::LocalAttachmentUnavailable)?
            .to_path_buf();
        let operation_cancellation = operation.cancellation().clone();
        let staged = tokio::task::spawn_blocking(move || {
            stage_delete_guard_restore(
                &key,
                &context,
                &guard_path,
                &destination_parent,
                &metadata,
                &operation_cancellation,
            )
        })
        .await
        .map_err(|_| Error::CacheUnavailable)??;
        Some((staged, destination, attachment.clone()))
    } else {
        None
    };
    operation.ensure_active()?;

    let mut staged =
        staged.map(|(file, destination, attachment)| (file, destination, attachment, Vec::new()));
    let mut retry_delays = [0, 50, 250, 1_000].into_iter();
    let mut last_retry_error = None;
    let (_synced_write_guard, mut transaction, current, current_attachment) = loop {
        let Some(delay_ms) = retry_delays.next() else {
            return Err(last_retry_error.unwrap_or(Error::CacheUnavailable));
        };
        operation.ensure_active()?;
        if delay_ms > 0 {
            tokio::select! {
                () = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
                () = operation.cancellation().cancelled() => return Err(Error::Cancelled),
            }
        }

        let synced_write_guard = state.synced_write_guard().await;
        let mut transaction = state.pool().begin_with("BEGIN IMMEDIATE").await?;
        let current = sqlx::query_as::<_, DeleteSourcePreflight>(DELETE_PREFLIGHT_SELECT)
            .bind(job_id)
            .bind(attempt_count)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(Error::DeleteGuardChanged)?;
        if !same_delete_job(&record, &current) || current.cache_id != guard_id {
            return Err(Error::DeleteGuardChanged);
        }
        let current_attachment = delete_source_attachment(&current)?;
        if current_attachment.is_some()
            && staged.as_ref().map(|(_, _, attachment, _)| attachment)
                != current_attachment.as_ref()
        {
            return Err(Error::DeleteGuardChanged);
        }

        if current_attachment.is_some() {
            let (file, destination, attachment, conflicts) =
                staged.take().ok_or(Error::DeleteGuardChanged)?;
            let expected_size = valid_plaintext_size(current.expected_size_bytes)?;
            match reconcile_staged_delete_guard_once(
                file,
                &destination,
                expected_size,
                &current.expected_sha256,
                conflicts,
            )? {
                DeleteGuardReconcile::Ready(conflicts) => drop(conflicts),
                DeleteGuardReconcile::Retry {
                    staged: file,
                    conflicts,
                    error,
                } => {
                    staged = Some((file, destination, attachment, conflicts));
                    last_retry_error = Some(error);
                    transaction.rollback().await?;
                    drop(synced_write_guard);
                    continue;
                }
            }
        }

        break (synced_write_guard, transaction, current, current_attachment);
    };
    operation.ensure_active()?;
    operation.begin_commit()?;

    if current_attachment.is_some() {
        let attachment = current_attachment
            .as_ref()
            .ok_or(Error::DeleteGuardChanged)?;
        let local_state = sqlx::query(
            "INSERT INTO attachment_local_state (
               attachment_id, session_id, relative_path, availability, updated_at
             )
             SELECT attachment.id, attachment.session_id, attachment.relative_path,
                    'present', strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             FROM session_attachments AS attachment
             JOIN attachment_transfer_jobs AS job
               ON job.id = ? AND job.attempt_count = ?
              AND job.direction = 'delete' AND job.phase = 'finalizing'
              AND job.attachment_id = attachment.id
              AND job.session_id = attachment.session_id
              AND job.workspace_id = attachment.workspace_id
              AND job.expected_sha256 = attachment.sha256
              AND job.expected_size_bytes = attachment.size_bytes
              AND job.object_key = attachment.cloud_object_key
              AND job.cache_id = ? AND job.ciphertext_sha256 = ?
              AND job.ciphertext_size_bytes = ?
             WHERE attachment.id = ? AND attachment.session_id = ?
               AND attachment.workspace_id = ? AND attachment.relative_path = ?
               AND attachment.source_type = ? AND attachment.deleted_at IS NULL
             ON CONFLICT(attachment_id) DO UPDATE SET
               session_id = excluded.session_id,
               relative_path = excluded.relative_path,
               availability = excluded.availability,
               updated_at = excluded.updated_at",
        )
        .bind(job_id)
        .bind(attempt_count)
        .bind(&current.cache_id)
        .bind(&current.ciphertext_sha256)
        .bind(current.ciphertext_size_bytes)
        .bind(&current.attachment_id)
        .bind(&current.session_id)
        .bind(&current.workspace_id)
        .bind(&attachment.relative_path)
        .bind(&attachment.source_type)
        .execute(&mut *transaction)
        .await?;
        if local_state.rows_affected() != 1 {
            return Err(Error::DeleteGuardChanged);
        }
    }

    sqlx::query(
        "UPDATE session_attachments
         SET storage_kind = 'local_file', cloud_object_key = '',
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND cloud_object_key = ?
           AND EXISTS (
             SELECT 1 FROM attachment_transfer_jobs AS job
             WHERE job.id = ? AND job.attempt_count = ?
               AND job.direction = 'delete' AND job.phase = 'finalizing'
               AND job.attachment_id = ? AND job.session_id = ?
               AND job.workspace_id = ? AND job.expected_sha256 = ?
               AND job.expected_size_bytes = ? AND job.object_key = ?
               AND job.cache_id = ? AND job.ciphertext_sha256 = ?
               AND job.ciphertext_size_bytes = ?
           )",
    )
    .bind(&current.attachment_id)
    .bind(&current.object_key)
    .bind(job_id)
    .bind(attempt_count)
    .bind(&current.attachment_id)
    .bind(&current.session_id)
    .bind(&current.workspace_id)
    .bind(&current.expected_sha256)
    .bind(current.expected_size_bytes)
    .bind(&current.object_key)
    .bind(&current.cache_id)
    .bind(&current.ciphertext_sha256)
    .bind(current.ciphertext_size_bytes)
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO attachment_transfer_jobs (
           id, attachment_id, session_id, workspace_id, direction,
           expected_sha256, expected_size_bytes
         )
         SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
                'upload', attachment.sha256, attachment.size_bytes
         FROM session_attachments AS attachment
         JOIN attachment_local_state AS local
           ON local.attachment_id = attachment.id AND local.availability = 'present'
         WHERE attachment.id = ? AND attachment.cloud_sync_enabled = 1
           AND attachment.cloud_object_key = '' AND attachment.deleted_at IS NULL
           AND EXISTS (
             SELECT 1 FROM attachment_transfer_jobs AS job
             WHERE job.id = ? AND job.attempt_count = ?
               AND job.direction = 'delete' AND job.phase = 'finalizing'
               AND job.attachment_id = ? AND job.session_id = ?
               AND job.workspace_id = ? AND job.expected_sha256 = ?
               AND job.expected_size_bytes = ? AND job.object_key = ?
               AND job.cache_id = ? AND job.ciphertext_sha256 = ?
               AND job.ciphertext_size_bytes = ?
           )",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&current.attachment_id)
    .bind(job_id)
    .bind(attempt_count)
    .bind(&current.attachment_id)
    .bind(&current.session_id)
    .bind(&current.workspace_id)
    .bind(&current.expected_sha256)
    .bind(current.expected_size_bytes)
    .bind(&current.object_key)
    .bind(&current.cache_id)
    .bind(&current.ciphertext_sha256)
    .bind(current.ciphertext_size_bytes)
    .execute(&mut *transaction)
    .await?;

    let completed = sqlx::query(
        "UPDATE attachment_transfer_jobs
         SET phase = 'completed', cache_id = '', ciphertext_sha256 = '',
             ciphertext_size_bytes = 0, last_error = '',
             completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND attempt_count = ? AND direction = 'delete'
           AND phase = 'finalizing' AND attachment_id = ? AND session_id = ?
           AND workspace_id = ? AND expected_sha256 = ?
           AND expected_size_bytes = ? AND object_key = ?
           AND cache_id = ? AND ciphertext_sha256 = ? AND ciphertext_size_bytes = ?",
    )
    .bind(job_id)
    .bind(attempt_count)
    .bind(&current.attachment_id)
    .bind(&current.session_id)
    .bind(&current.workspace_id)
    .bind(&current.expected_sha256)
    .bind(current.expected_size_bytes)
    .bind(&current.object_key)
    .bind(&current.cache_id)
    .bind(&current.ciphertext_sha256)
    .bind(current.ciphertext_size_bytes)
    .execute(&mut *transaction)
    .await?;
    if completed.rows_affected() != 1 {
        return Err(Error::DeleteGuardChanged);
    }
    transaction.commit().await?;
    drop(_synced_write_guard);
    cleanup_linked_delete_guard(app, guard_id).await;
    Ok(())
}

pub async fn reconcile_delete_guards<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri_plugin_db::ManagedState,
) -> Result<u64> {
    let referenced = sqlx::query_scalar::<_, String>(
        "SELECT cache_id FROM attachment_transfer_jobs
         WHERE direction = 'delete' AND phase <> 'completed' AND cache_id <> ''",
    )
    .fetch_all(state.pool())
    .await?
    .into_iter()
    .filter(|cache_id| valid_cache_id(cache_id))
    .collect::<HashSet<_>>();
    let root = delete_guard_root(app)?;
    let orphan_before = SystemTime::now()
        .checked_sub(DELETE_GUARD_ORPHAN_GRACE)
        .ok_or(Error::CacheUnavailable)?;
    tokio::task::spawn_blocking(move || {
        reconcile_delete_guard_files(&root, &referenced, orphan_before)
    })
    .await
    .map_err(|_| Error::CacheUnavailable)?
}

#[allow(clippy::too_many_arguments)]
pub async fn download_and_restore<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri_plugin_db::ManagedState,
    operation: &DownloadOperation,
    job_id: &str,
    attempt_count: i64,
    object_id: &str,
    signed_url: &str,
    ciphertext_sha256: &str,
    ciphertext_size_bytes: u64,
    format_version: i16,
) -> Result<RestoredAttachment> {
    operation.ensure_active()?;
    if format_version != FORMAT_VERSION
        || !valid_sha256(ciphertext_sha256)
        || ciphertext_size_bytes == 0
        || ciphertext_size_bytes > MAX_CIPHERTEXT_BYTES
    {
        return Err(Error::InvalidMetadata);
    }
    let record =
        load_transfer_attachment(state.pool(), job_id, attempt_count, "download", false).await?;
    validate_transfer_version(&record)?;
    if record.object_key != record.attachment_cloud_object_key {
        return Err(Error::InvalidTransferState);
    }
    validate_object_identity(object_id, &record.object_key)?;
    if !record.remote_object_id.is_empty() && record.remote_object_id != object_id {
        return Err(Error::InvalidTransferState);
    }
    let expected_plaintext =
        plaintext_metadata(&record.expected_sha256, record.expected_size_bytes)?;
    let key = workspace_key(state, &record.workspace_id)?;
    let predicted_size = key.attachment_blob_ciphertext_size(
        &record.workspace_id,
        &record.attachment_id,
        expected_plaintext.size_bytes,
    )?;
    if predicted_size != ciphertext_size_bytes {
        return Err(Error::InvalidMetadata);
    }
    let download_url = validate_signed_download_url(
        signed_url,
        require_configured_supabase_url(crate::configured_supabase_url())?,
        DownloadObject::Private(&record.object_key),
    )?;
    let cache_root = private_cache_root(app)?;
    tokio::fs::create_dir_all(&cache_root).await?;
    let cache_id = Uuid::new_v4().to_string();
    let cache_path = private_cache_path(&cache_root, &cache_id)?;
    let _cache_guard = CacheFileGuard::new(cache_path.clone());

    let updated = sqlx::query(
        "UPDATE attachment_transfer_jobs
         SET remote_object_id = ?, cache_id = ?, ciphertext_sha256 = ?,
             ciphertext_size_bytes = ?, phase = 'transferring',
             last_attempt_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND direction = 'download' AND phase <> 'completed'
           AND attachment_id = ? AND session_id = ? AND workspace_id = ?
           AND expected_sha256 = ? AND expected_size_bytes = ?
           AND remote_object_id = ? AND object_key = ?
           AND cache_id = ? AND phase = ? AND attempt_count = ?",
    )
    .bind(object_id)
    .bind(&cache_id)
    .bind(ciphertext_sha256)
    .bind(i64::try_from(ciphertext_size_bytes).map_err(|_| Error::InvalidMetadata)?)
    .bind(job_id)
    .bind(&record.attachment_id)
    .bind(&record.session_id)
    .bind(&record.workspace_id)
    .bind(&record.expected_sha256)
    .bind(record.expected_size_bytes)
    .bind(&record.remote_object_id)
    .bind(&record.object_key)
    .bind(&record.cache_id)
    .bind(&record.phase)
    .bind(attempt_count)
    .execute(state.pool())
    .await?;
    if updated.rows_affected() != 1 {
        return Err(Error::InvalidTransferState);
    }

    if record.cache_id != cache_id && valid_cache_id(&record.cache_id) {
        let old_path = private_cache_path(&cache_root, &record.cache_id)?;
        let _ = tokio::fs::remove_file(old_path).await;
    }

    let result = async {
        download_to_path(
            download_url,
            &cache_path,
            ciphertext_size_bytes,
            ciphertext_sha256,
            true,
            operation.cancellation(),
        )
        .await?;

        let destination = resolve_attachment_path(app, &record.local_attachment(), false)?;
        let context = AttachmentBlobContext::new(
            record.workspace_id.clone(),
            record.attachment_id.clone(),
            object_id.to_string(),
        )?;
        let expected_blob = AttachmentBlobMetadata {
            version: u8::try_from(format_version).map_err(|_| Error::InvalidMetadata)?,
            plaintext: expected_plaintext.clone(),
            ciphertext: AttachmentBlobCiphertextMetadata::from_hex(
                ciphertext_size_bytes,
                ciphertext_sha256,
            )?,
        };
        let cache_path_for_open = cache_path.clone();
        let destination_parent = destination
            .parent()
            .ok_or(Error::LocalAttachmentUnavailable)?
            .to_path_buf();
        let staged = tokio::task::spawn_blocking(move || {
            stage_attachment_restore(
                &key,
                &context,
                &cache_path_for_open,
                &destination_parent,
                &expected_blob,
            )
        })
        .await
        .map_err(|_| Error::CacheUnavailable)??;

        let mut transaction = state.pool().begin_with("BEGIN IMMEDIATE").await?;
        let canonical: bool = sqlx::query_scalar(
            "SELECT EXISTS(
               SELECT 1
               FROM session_attachments AS attachment
               JOIN attachment_transfer_jobs AS job
                 ON job.attachment_id = attachment.id
                AND job.session_id = attachment.session_id
                AND job.workspace_id = attachment.workspace_id
               WHERE attachment.id = ? AND attachment.session_id = ?
                 AND attachment.workspace_id = ? AND attachment.sha256 = ?
                 AND attachment.size_bytes = ? AND attachment.deleted_at IS NULL
                 AND attachment.relative_path = ? AND attachment.source_type = ?
                 AND attachment.cloud_object_key = ?
                 AND job.id = ? AND job.direction = 'download'
                 AND job.object_key = ? AND job.remote_object_id = ?
                 AND job.cache_id = ? AND job.phase = 'transferring'
                 AND job.attempt_count = ?
             )",
        )
        .bind(&record.attachment_id)
        .bind(&record.session_id)
        .bind(&record.workspace_id)
        .bind(&record.expected_sha256)
        .bind(record.expected_size_bytes)
        .bind(&record.relative_path)
        .bind(&record.source_type)
        .bind(&record.object_key)
        .bind(job_id)
        .bind(&record.object_key)
        .bind(object_id)
        .bind(&cache_id)
        .bind(attempt_count)
        .fetch_one(&mut *transaction)
        .await?;
        if !canonical {
            return Err(Error::InvalidTransferState);
        }
        operation.begin_commit()?;
        persist_staged_attachment(staged, &destination)?;

        let local_state = sqlx::query(
            "INSERT INTO attachment_local_state (
               attachment_id, session_id, relative_path, availability, updated_at
             )
             SELECT id, session_id, relative_path, 'present',
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             FROM session_attachments
             WHERE id = ? AND session_id = ? AND workspace_id = ?
               AND sha256 = ? AND size_bytes = ? AND deleted_at IS NULL
               AND relative_path = ? AND source_type = ?
               AND cloud_object_key = ?
             ON CONFLICT(attachment_id) DO UPDATE SET
               session_id = excluded.session_id,
               relative_path = excluded.relative_path,
               availability = excluded.availability,
               updated_at = excluded.updated_at",
        )
        .bind(&record.attachment_id)
        .bind(&record.session_id)
        .bind(&record.workspace_id)
        .bind(&record.expected_sha256)
        .bind(record.expected_size_bytes)
        .bind(&record.relative_path)
        .bind(&record.source_type)
        .bind(&record.object_key)
        .execute(&mut *transaction)
        .await?;
        let completed = sqlx::query(
            "UPDATE attachment_transfer_jobs
             SET phase = 'completed', cache_id = '', last_error = '',
                 completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ? AND direction = 'download' AND cache_id = ?
               AND remote_object_id = ? AND phase = 'transferring'
               AND attempt_count = ?",
        )
        .bind(job_id)
        .bind(&cache_id)
        .bind(object_id)
        .bind(attempt_count)
        .execute(&mut *transaction)
        .await?;
        if local_state.rows_affected() != 1 || completed.rows_affected() != 1 {
            return Err(Error::InvalidTransferState);
        }
        transaction.commit().await?;

        Ok(RestoredAttachment {
            attachment_id: record.attachment_id.clone(),
            session_id: record.session_id.clone(),
            relative_path: record.relative_path.clone(),
            size_bytes: expected_plaintext.size_bytes,
            sha256: expected_plaintext.sha256_hex(),
        })
    }
    .await;

    let _ = tokio::fs::remove_file(&cache_path).await;
    if result.is_err() {
        let _ = sqlx::query(
            "UPDATE attachment_transfer_jobs
             SET cache_id = '', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ? AND direction = 'download' AND cache_id = ?
               AND remote_object_id = ? AND phase = 'transferring'
               AND attempt_count = ?",
        )
        .bind(job_id)
        .bind(&cache_id)
        .bind(object_id)
        .bind(attempt_count)
        .execute(state.pool())
        .await;
    }

    result
}

pub async fn cleanup_transfer_cache<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri_plugin_db::ManagedState,
    job_id: &str,
    attempt_count: i64,
    expected_cache_id: &str,
) -> Result<bool> {
    validate_opaque_id(job_id)?;
    if attempt_count <= 0 || !valid_cache_id(expected_cache_id) {
        return Err(Error::InvalidTransferState);
    }
    let current: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1
           FROM attachment_transfer_jobs
           WHERE id = ? AND attempt_count = ? AND cache_id = ?
         )",
    )
    .bind(job_id)
    .bind(attempt_count)
    .bind(expected_cache_id)
    .fetch_one(state.pool())
    .await?;
    if !current {
        return Ok(false);
    }

    let path = private_cache_path(&private_cache_root(app)?, expected_cache_id)?;
    let removed = match tokio::fs::remove_file(path).await {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    sqlx::query(
        "UPDATE attachment_transfer_jobs
         SET cache_id = '', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND attempt_count = ? AND cache_id = ?",
    )
    .bind(job_id)
    .bind(attempt_count)
    .bind(expected_cache_id)
    .execute(state.pool())
    .await?;
    Ok(removed)
}

#[allow(clippy::too_many_arguments)]
pub async fn download_shared_attachment<R: Runtime>(
    app: &tauri::AppHandle<R>,
    operation: &DownloadOperation,
    scope_id: &str,
    attachment_id: &str,
    signed_url: &str,
    expected_sha256: &str,
    expected_size_bytes: u64,
) -> Result<SharedAttachmentCacheResult> {
    operation.ensure_active()?;
    validate_opaque_id(scope_id)?;
    validate_uuid_v4(attachment_id)?;
    if !valid_sha256(expected_sha256) || expected_size_bytes > MAX_PLAINTEXT_BYTES {
        return Err(Error::InvalidMetadata);
    }
    let url = validate_signed_download_url(
        signed_url,
        require_configured_supabase_url(crate::configured_supabase_url())?,
        DownloadObject::Shared(attachment_id),
    )?;
    let scope_path = shared_scope_path(app, scope_id)?;
    tokio::fs::create_dir_all(&scope_path).await?;
    let cache_id = shared_cache_id(scope_id, attachment_id);
    let local_path = cached_shared_attachment_path(&scope_path, &cache_id);

    if shared_cache_matches_async(
        scope_path.clone(),
        cache_id.clone(),
        expected_size_bytes,
        expected_sha256.to_string(),
    )
    .await?
    {
        operation.ensure_active()?;
        return Ok(SharedAttachmentCacheResult {
            cache_id,
            local_path: local_path.to_string_lossy().into_owned(),
            size_bytes: expected_size_bytes,
            sha256: expected_sha256.to_string(),
        });
    }

    let temp = tempfile::NamedTempFile::new_in(&scope_path)?;
    let temp_path = temp.path().to_path_buf();
    download_to_path(
        url,
        &temp_path,
        expected_size_bytes,
        expected_sha256,
        false,
        operation.cancellation(),
    )
    .await?;
    temp.as_file().sync_all()?;
    operation.begin_commit()?;
    temp.persist(&local_path)
        .map_err(|error| Error::Io(error.error))?;
    commit_shared_cache_entry(
        &scope_path,
        &cache_id,
        &local_path,
        expected_size_bytes,
        expected_sha256,
    )?;

    Ok(SharedAttachmentCacheResult {
        cache_id,
        local_path: local_path.to_string_lossy().into_owned(),
        size_bytes: expected_size_bytes,
        sha256: expected_sha256.to_string(),
    })
}

pub async fn existing_shared_attachment_path<R: Runtime>(
    app: &tauri::AppHandle<R>,
    scope_id: &str,
    attachment_id: &str,
) -> Result<Option<String>> {
    validate_opaque_id(scope_id)?;
    validate_opaque_id(attachment_id)?;
    let scope_path = shared_scope_path(app, scope_id)?;
    let cache_id = shared_cache_id(scope_id, attachment_id);
    let Some((expected_size, expected_sha256)) =
        read_shared_cache_metadata(&scope_path, &cache_id)?
    else {
        return Ok(None);
    };
    if !shared_cache_matches_async(
        scope_path.clone(),
        cache_id.clone(),
        expected_size,
        expected_sha256,
    )
    .await?
    {
        return Ok(None);
    }
    Ok(Some(
        cached_shared_attachment_path(&scope_path, &cache_id)
            .to_string_lossy()
            .into_owned(),
    ))
}

pub async fn remove_shared_attachment<R: Runtime>(
    app: &tauri::AppHandle<R>,
    scope_id: &str,
    attachment_id: &str,
) -> Result<bool> {
    validate_opaque_id(scope_id)?;
    validate_opaque_id(attachment_id)?;
    let scope_path = shared_scope_path(app, scope_id)?;
    let cache_id = shared_cache_id(scope_id, attachment_id);
    let data_path = cached_shared_attachment_path(&scope_path, &cache_id);
    let metadata_path = shared_cache_metadata_path(&scope_path, &cache_id);
    let removed = match tokio::fs::remove_file(data_path).await {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    match tokio::fs::remove_file(metadata_path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(removed)
}

pub async fn clear_shared_attachment_scope<R: Runtime>(
    app: &tauri::AppHandle<R>,
    scope_id: &str,
) -> Result<u64> {
    validate_opaque_id(scope_id)?;
    let path = shared_scope_path(app, scope_id)?;
    let mut count = 0_u64;
    let mut entries = match tokio::fs::read_dir(&path).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        if (file_type.is_file() || file_type.is_symlink())
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "bin")
        {
            count = count.saturating_add(1);
        }
    }
    drop(entries);
    match tokio::fs::remove_dir_all(path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(count)
}

pub(crate) fn clear_shared_attachment_cache_root<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<()> {
    clear_attachment_cache_directory(&shared_cache_root(app)?)
}

pub(crate) fn clear_private_attachment_cache_root<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<()> {
    clear_attachment_cache_directory(&private_cache_root(app)?)
}

pub(crate) fn clear_shared_upload_cache_root<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<()> {
    clear_attachment_cache_directory(&shared_upload_cache_root(app)?)
}

impl TransferAttachment {
    fn local_attachment(&self) -> LocalAttachment {
        LocalAttachment {
            attachment_id: self.attachment_id.clone(),
            session_id: self.session_id.clone(),
            workspace_id: self.workspace_id.clone(),
            relative_path: self.relative_path.clone(),
            source_type: self.source_type.clone(),
            sha256: self.attachment_sha256.clone(),
            size_bytes: self.attachment_size_bytes,
        }
    }
}

impl SharedUploadAttachment {
    fn local_attachment(&self) -> LocalAttachment {
        LocalAttachment {
            attachment_id: self.attachment_id.clone(),
            session_id: self.session_id.clone(),
            workspace_id: self.workspace_id.clone(),
            relative_path: self.relative_path.clone(),
            source_type: self.source_type.clone(),
            sha256: self.sha256.clone(),
            size_bytes: self.size_bytes,
        }
    }
}

fn delete_source_attachment(record: &DeleteSourcePreflight) -> Result<Option<LocalAttachment>> {
    let Some(attachment_id) = record.current_attachment_id.as_ref() else {
        return Ok(None);
    };
    if record.deleted_at.is_some() {
        return Ok(None);
    }
    let cloud_object_key = record
        .attachment_cloud_object_key
        .as_ref()
        .ok_or(Error::InvalidTransferState)?;
    if cloud_object_key != &record.object_key {
        return Ok(None);
    }
    let sha256 = record
        .attachment_sha256
        .as_ref()
        .ok_or(Error::InvalidTransferState)?;
    let size_bytes = record
        .attachment_size_bytes
        .ok_or(Error::InvalidTransferState)?;
    if sha256 != &record.expected_sha256 || size_bytes != record.expected_size_bytes {
        return Ok(None);
    }
    if !matches!(record.cloud_sync_enabled, Some(0 | 1)) {
        return Err(Error::InvalidTransferState);
    }
    Ok(Some(LocalAttachment {
        attachment_id: attachment_id.clone(),
        session_id: record.session_id.clone(),
        workspace_id: record.workspace_id.clone(),
        relative_path: record
            .relative_path
            .clone()
            .ok_or(Error::InvalidTransferState)?,
        source_type: record
            .source_type
            .clone()
            .ok_or(Error::InvalidTransferState)?,
        sha256: sha256.clone(),
        size_bytes,
    }))
}

fn same_delete_job(left: &DeleteSourcePreflight, right: &DeleteSourcePreflight) -> bool {
    left.attachment_id == right.attachment_id
        && left.session_id == right.session_id
        && left.workspace_id == right.workspace_id
        && left.expected_sha256 == right.expected_sha256
        && left.expected_size_bytes == right.expected_size_bytes
        && left.object_key == right.object_key
        && left.cache_id == right.cache_id
        && left.ciphertext_sha256 == right.ciphertext_sha256
        && left.ciphertext_size_bytes == right.ciphertext_size_bytes
}

async fn clear_delete_guard_link(
    pool: &sqlx::SqlitePool,
    operation: &DownloadOperation,
    job_id: &str,
    attempt_count: i64,
    record: &DeleteSourcePreflight,
) -> Result<()> {
    operation.ensure_active()?;
    if record.cache_id.is_empty()
        && record.ciphertext_sha256.is_empty()
        && record.ciphertext_size_bytes == 0
    {
        return Ok(());
    }
    operation.begin_commit()?;
    let cleared = sqlx::query(
        "UPDATE attachment_transfer_jobs
         SET cache_id = '', ciphertext_sha256 = '', ciphertext_size_bytes = 0,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND attempt_count = ?
           AND direction = 'delete' AND phase = 'finalizing'
           AND attachment_id = ? AND session_id = ? AND workspace_id = ?
           AND expected_sha256 = ? AND expected_size_bytes = ? AND object_key = ?
           AND cache_id = ? AND ciphertext_sha256 = ? AND ciphertext_size_bytes = ?",
    )
    .bind(job_id)
    .bind(attempt_count)
    .bind(&record.attachment_id)
    .bind(&record.session_id)
    .bind(&record.workspace_id)
    .bind(&record.expected_sha256)
    .bind(record.expected_size_bytes)
    .bind(&record.object_key)
    .bind(&record.cache_id)
    .bind(&record.ciphertext_sha256)
    .bind(record.ciphertext_size_bytes)
    .execute(pool)
    .await?;
    if cleared.rows_affected() != 1 {
        return Err(Error::DeleteGuardChanged);
    }
    Ok(())
}

fn delete_guard_metadata(
    record: &DeleteSourcePreflight,
    key: &WorkspaceKey,
    plaintext: &AttachmentBlobPlaintextMetadata,
) -> Result<Option<AttachmentBlobMetadata>> {
    if record.cache_id.is_empty()
        || !valid_cache_id(&record.cache_id)
        || !valid_sha256(&record.ciphertext_sha256)
    {
        return Ok(None);
    }
    let ciphertext_size = match u64::try_from(record.ciphertext_size_bytes) {
        Ok(size) if size > 0 && size <= MAX_CIPHERTEXT_BYTES => size,
        _ => return Ok(None),
    };
    let expected_size = key.attachment_blob_ciphertext_size(
        &record.workspace_id,
        &record.attachment_id,
        plaintext.size_bytes,
    )?;
    if ciphertext_size != expected_size {
        return Ok(None);
    }
    Ok(Some(AttachmentBlobMetadata {
        version: u8::try_from(FORMAT_VERSION).map_err(|_| Error::InvalidMetadata)?,
        plaintext: plaintext.clone(),
        ciphertext: AttachmentBlobCiphertextMetadata::from_hex(
            ciphertext_size,
            &record.ciphertext_sha256,
        )?,
    }))
}

fn delete_source_changed(error: &Error) -> bool {
    match error {
        Error::LocalAttachmentUnavailable
        | Error::ChecksumMismatch
        | Error::E2ee(hypr_e2ee::AttachmentBlobError::SourceMismatch) => true,
        Error::Io(source) => source.kind() == std::io::ErrorKind::NotFound,
        _ => false,
    }
}

async fn load_transfer_attachment(
    pool: &sqlx::SqlitePool,
    job_id: &str,
    attempt_count: i64,
    direction: &str,
    require_local: bool,
) -> Result<TransferAttachment> {
    validate_opaque_id(job_id)?;
    if attempt_count <= 0 {
        return Err(Error::InvalidTransferState);
    }
    let record = sqlx::query_as::<_, TransferAttachment>(
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
           attachment.cloud_object_key AS attachment_cloud_object_key,
           attachment.cloud_sync_enabled
         FROM attachment_transfer_jobs AS job
         JOIN session_attachments AS attachment
           ON attachment.id = job.attachment_id
          AND attachment.session_id = job.session_id
          AND attachment.workspace_id = job.workspace_id
          AND attachment.deleted_at IS NULL
         LEFT JOIN attachment_local_state AS local
           ON local.attachment_id = attachment.id
         WHERE job.id = ? AND job.attempt_count = ?
           AND job.direction = ? AND job.phase <> 'completed'
           AND (? = 0 OR local.availability = 'present')
         LIMIT 1",
    )
    .bind(job_id)
    .bind(attempt_count)
    .bind(direction)
    .bind(i64::from(require_local))
    .fetch_optional(pool)
    .await?;
    record.ok_or(if require_local {
        Error::LocalAttachmentUnavailable
    } else {
        Error::InvalidTransferState
    })
}

async fn load_shared_upload_attachment(
    pool: &sqlx::SqlitePool,
    attachment_id: &str,
) -> Result<SharedUploadAttachment> {
    validate_opaque_id(attachment_id)?;
    sqlx::query_as::<_, SharedUploadAttachment>(
        "SELECT
           attachment.id AS attachment_id,
           attachment.session_id,
           attachment.workspace_id,
           attachment.relative_path,
           attachment.source_type,
           attachment.sha256,
           attachment.size_bytes,
           attachment.filename,
           attachment.content_type,
           attachment.cloud_sync_enabled,
           attachment.cloud_object_key
         FROM session_attachments AS attachment
         JOIN attachment_local_state AS local
           ON local.attachment_id = attachment.id
          AND local.availability = 'present'
         WHERE attachment.id = ? AND attachment.deleted_at IS NULL
         LIMIT 1",
    )
    .bind(attachment_id)
    .fetch_optional(pool)
    .await?
    .ok_or(Error::LocalAttachmentUnavailable)
}

fn validate_shared_upload_version(
    attachment: &SharedUploadAttachment,
    expected_sha256: &str,
    expected_size_bytes: u64,
    expected_filename: &str,
    expected_content_type: &str,
    expected_cloud_object_key: &str,
) -> Result<()> {
    if !valid_sha256(expected_sha256)
        || expected_size_bytes == 0
        || expected_size_bytes > MAX_PLAINTEXT_BYTES
        || attachment.sha256 != expected_sha256
        || u64::try_from(attachment.size_bytes).ok() != Some(expected_size_bytes)
        || attachment.filename != expected_filename
        || attachment.content_type != expected_content_type
        || attachment.cloud_sync_enabled != 1
        || expected_cloud_object_key.is_empty()
        || attachment.cloud_object_key != expected_cloud_object_key
    {
        return Err(Error::InvalidTransferState);
    }
    Ok(())
}

fn validate_transfer_version(record: &TransferAttachment) -> Result<()> {
    if record.job_id.is_empty()
        || record.expected_sha256 != record.attachment_sha256
        || record.expected_size_bytes != record.attachment_size_bytes
        || !matches!(record.cloud_sync_enabled, 0 | 1)
    {
        return Err(Error::InvalidTransferState);
    }
    plaintext_metadata(&record.expected_sha256, record.expected_size_bytes)?;
    Ok(())
}

fn validate_upload_transfer_version(record: &TransferAttachment) -> Result<()> {
    validate_transfer_version(record)?;
    if record.cloud_sync_enabled != 1 {
        return Err(Error::InvalidTransferState);
    }
    Ok(())
}

fn plaintext_metadata(sha256: &str, size_bytes: i64) -> Result<AttachmentBlobPlaintextMetadata> {
    let size_bytes = valid_plaintext_size(size_bytes)?;
    if !valid_sha256(sha256) {
        return Err(Error::InvalidMetadata);
    }
    AttachmentBlobPlaintextMetadata::from_hex(size_bytes, sha256).map_err(Into::into)
}

fn valid_plaintext_size(value: i64) -> Result<u64> {
    let value = u64::try_from(value).map_err(|_| Error::InvalidMetadata)?;
    if value > MAX_PLAINTEXT_BYTES {
        return Err(Error::InvalidMetadata);
    }
    Ok(value)
}

fn workspace_key(
    state: &tauri_plugin_db::ManagedState,
    workspace_id: &str,
) -> Result<WorkspaceKey> {
    state
        .workspace_key(workspace_id)
        .ok_or(Error::WorkspaceKeyUnavailable)
}

fn resolve_attachment_path<R: Runtime>(
    app: &tauri::AppHandle<R>,
    attachment: &LocalAttachment,
    must_exist: bool,
) -> Result<PathBuf> {
    let (session_dir, path) = attachment_paths(app, attachment)?;
    if must_exist {
        if !path.is_file() {
            return Err(Error::LocalAttachmentUnavailable);
        }
    } else {
        std::fs::create_dir_all(session_dir)?;
    }
    Ok(path)
}

fn attachment_paths<R: Runtime>(
    app: &tauri::AppHandle<R>,
    attachment: &LocalAttachment,
) -> Result<(PathBuf, PathBuf)> {
    validate_attachment_relative_path(&attachment.source_type, &attachment.relative_path)?;
    if attachment.attachment_id.is_empty()
        || attachment.session_id.is_empty()
        || attachment.workspace_id.is_empty()
        || !valid_sha256(&attachment.sha256)
    {
        return Err(Error::InvalidMetadata);
    }
    let vault_base = app
        .settings()
        .vault_base()
        .map_err(|_| Error::Vault)?
        .into_std_path_buf();
    let session_candidate = hypr_fs_sync_core::FsSyncCore::new(vault_base.clone())
        .resolve_session_dir(&attachment.session_id)
        .map_err(|_| Error::LocalAttachmentUnavailable)?;
    let session_dir = hypr_fs_sync_core::resolve_path_inside_base(&vault_base, &session_candidate)
        .map_err(|_| Error::LocalAttachmentUnavailable)?;
    let path = hypr_fs_sync_core::resolve_path_inside_base(
        &session_dir,
        Path::new(&attachment.relative_path),
    )
    .map_err(|_| Error::LocalAttachmentUnavailable)?;
    Ok((session_dir, path))
}

fn validate_attachment_relative_path(source_type: &str, relative_path: &str) -> Result<()> {
    let components = Path::new(relative_path).components().collect::<Vec<_>>();
    let valid = if source_type == "session_audio" {
        matches!(
            components.as_slice(),
            [Component::Normal(name)]
                if matches!(name.to_str(), Some("audio.mp3" | "audio.wav" | "audio.ogg"))
        )
    } else {
        matches!(
            components.as_slice(),
            [Component::Normal(directory), Component::Normal(filename)]
                if directory == &std::ffi::OsStr::new("attachments")
                    && !filename.is_empty()
        )
    };
    if !valid {
        return Err(Error::InvalidMetadata);
    }
    Ok(())
}

fn validate_object_identity(object_id: &str, object_key: &str) -> Result<()> {
    validate_uuid_v4(object_id)?;
    let (owner, filename) = object_key.split_once('/').ok_or(Error::InvalidMetadata)?;
    let owner_uuid = Uuid::parse_str(owner).map_err(|_| Error::InvalidMetadata)?;
    if owner_uuid.to_string() != owner
        || filename != format!("{object_id}.anb1")
        || filename.contains('/')
    {
        return Err(Error::InvalidMetadata);
    }
    Ok(())
}

fn private_object_id(object_key: &str) -> Result<String> {
    let (_, filename) = object_key.split_once('/').ok_or(Error::InvalidMetadata)?;
    let object_id = filename
        .strip_suffix(".anb1")
        .ok_or(Error::InvalidMetadata)?;
    validate_object_identity(object_id, object_key)?;
    Ok(object_id.to_string())
}

fn validate_uuid_v4(value: &str) -> Result<()> {
    let uuid = Uuid::parse_str(value).map_err(|_| Error::InvalidMetadata)?;
    if uuid.to_string() != value || uuid.get_version() != Some(Version::Random) {
        return Err(Error::InvalidMetadata);
    }
    Ok(())
}

fn validate_opaque_id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 512
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(Error::InvalidMetadata);
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_cache_id(value: &str) -> bool {
    Uuid::parse_str(value)
        .is_ok_and(|uuid| uuid.to_string() == value && uuid.get_version() == Some(Version::Random))
}

fn validate_range(start: u64, end: u64) -> Result<()> {
    if end <= start || end - start > MAX_RANGE_BYTES {
        return Err(Error::InvalidRange);
    }
    Ok(())
}

async fn read_range(path: PathBuf, start: u64, end: u64, expected_size: u64) -> Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(path)?;
        if file.metadata()?.len() != expected_size {
            return Err(Error::CacheUnavailable);
        }
        file.seek(SeekFrom::Start(start))?;
        let length = usize::try_from(end - start).map_err(|_| Error::InvalidRange)?;
        let mut bytes = vec![0_u8; length];
        file.read_exact(&mut bytes)?;
        Ok(bytes)
    })
    .await
    .map_err(|_| Error::CacheUnavailable)?
}

fn ensure_file_size(path: &Path, expected_size: u64) -> Result<()> {
    if std::fs::metadata(path)?.len() != expected_size {
        return Err(Error::LocalAttachmentUnavailable);
    }
    Ok(())
}

fn file_matches(path: &Path, expected_size: u64, expected_sha256: &str) -> Result<bool> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() != expected_size {
        return Ok(false);
    }
    Ok(hash_file(path)? == expected_sha256)
}

async fn file_matches_async(
    path: PathBuf,
    expected_size: u64,
    expected_sha256: String,
) -> Result<bool> {
    tokio::task::spawn_blocking(move || file_matches(&path, expected_size, &expected_sha256))
        .await
        .map_err(|_| Error::CacheUnavailable)?
}

async fn file_matches_cancellable_async(
    path: PathBuf,
    expected_size: u64,
    expected_sha256: String,
    cancellation: tokio_util::sync::CancellationToken,
) -> Result<bool> {
    tokio::task::spawn_blocking(move || {
        file_matches_cancellable(&path, expected_size, &expected_sha256, &cancellation)
    })
    .await
    .map_err(|_| Error::CacheUnavailable)?
}

fn file_matches_cancellable(
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<bool> {
    if cancellation.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() != expected_size {
        return Ok(false);
    }
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    if cancellation.is_cancelled() {
        return Err(Error::Cancelled);
    }
    Ok(hex_digest(hasher.finalize().as_slice()) == expected_sha256)
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(hasher.finalize().as_slice()))
}

fn snapshot_verified_file(
    source_path: &Path,
    cache_path: &Path,
    expected_size: u64,
    expected_sha256: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<SharedUploadCacheFileGuard> {
    if cancellation.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let mut source = std::fs::File::open(source_path)?;
    let source_metadata = source.metadata()?;
    if !source_metadata.is_file() || source_metadata.len() != expected_size {
        return Err(Error::LocalAttachmentUnavailable);
    }
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let cache_guard = SharedUploadCacheFileGuard::new(cache_path.to_path_buf());
    let mut cache = options.open(cache_path)?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        total = total
            .checked_add(read as u64)
            .ok_or(Error::ChecksumMismatch)?;
        if total > expected_size {
            return Err(Error::ChecksumMismatch);
        }
        hasher.update(&buffer[..read]);
        cache.write_all(&buffer[..read])?;
    }
    if cancellation.is_cancelled() {
        return Err(Error::Cancelled);
    }
    cache.sync_all()?;
    if total != expected_size || hex_digest(hasher.finalize().as_slice()) != expected_sha256 {
        return Err(Error::ChecksumMismatch);
    }
    Ok(cache_guard)
}

fn seal_delete_guard(
    key: &WorkspaceKey,
    context: &AttachmentBlobContext,
    source_path: &Path,
    guard_path: &Path,
    expected: &AttachmentBlobPlaintextMetadata,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<(AttachmentBlobMetadata, DeleteGuardFileGuard)> {
    let guard = DeleteGuardFileGuard::new(guard_path.to_path_buf());
    let mut source = CancellableReader {
        inner: std::fs::File::open(source_path)?,
        cancellation: cancellation.clone(),
    };
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut destination = options.open(guard_path)?;
    let metadata = match key.seal_attachment_blob(context, &mut source, &mut destination, expected)
    {
        Ok(metadata) => metadata,
        Err(hypr_e2ee::AttachmentBlobError::Io(error))
            if error.kind() == std::io::ErrorKind::ConnectionAborted =>
        {
            return Err(Error::Cancelled);
        }
        Err(error) => return Err(error.into()),
    };
    destination.sync_all()?;
    Ok((metadata, guard))
}

enum DeleteGuardReconcile {
    Ready(Vec<PathBuf>),
    Retry {
        staged: tempfile::NamedTempFile,
        conflicts: Vec<PathBuf>,
        error: Error,
    },
}

#[cfg(test)]
fn reconcile_staged_delete_guard(
    staged: tempfile::NamedTempFile,
    destination: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<Vec<PathBuf>> {
    match reconcile_staged_delete_guard_once(
        staged,
        destination,
        expected_size,
        expected_sha256,
        Vec::new(),
    )? {
        DeleteGuardReconcile::Ready(conflicts) => Ok(conflicts),
        DeleteGuardReconcile::Retry { error, .. } => Err(error),
    }
}

fn reconcile_staged_delete_guard_once(
    staged: tempfile::NamedTempFile,
    destination: &Path,
    expected_size: u64,
    expected_sha256: &str,
    mut conflicts: Vec<PathBuf>,
) -> Result<DeleteGuardReconcile> {
    let parent = destination
        .parent()
        .ok_or(Error::LocalAttachmentUnavailable)?;
    if let Err(error) = std::fs::create_dir_all(parent) {
        return Ok(DeleteGuardReconcile::Retry {
            staged,
            conflicts,
            error: error.into(),
        });
    }

    match regular_file_matches(destination, expected_size, expected_sha256) {
        Ok(true) => return Ok(DeleteGuardReconcile::Ready(conflicts)),
        Ok(false) => {}
        Err(error) => {
            return Ok(DeleteGuardReconcile::Retry {
                staged,
                conflicts,
                error,
            });
        }
    }
    match std::fs::symlink_metadata(destination) {
        Ok(_) => {
            let conflict = unique_attachment_conflict_path(destination)?;
            match std::fs::rename(destination, &conflict) {
                Ok(()) => {
                    conflicts.push(conflict);
                    if let Err(error) = sync_destination_directory(parent) {
                        return Ok(DeleteGuardReconcile::Retry {
                            staged,
                            conflicts,
                            error,
                        });
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Ok(DeleteGuardReconcile::Retry {
                        staged,
                        conflicts,
                        error: error.into(),
                    });
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Ok(DeleteGuardReconcile::Retry {
                staged,
                conflicts,
                error: error.into(),
            });
        }
    }
    match staged.persist_noclobber(destination) {
        Ok(_) => {
            sync_destination_directory(parent)?;
            Ok(DeleteGuardReconcile::Ready(conflicts))
        }
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            Ok(DeleteGuardReconcile::Retry {
                staged: error.file,
                conflicts,
                error: Error::DeleteGuardChanged,
            })
        }
        Err(error) => Ok(DeleteGuardReconcile::Retry {
            staged: error.file,
            conflicts,
            error: Error::Io(error.error),
        }),
    }
}

fn regular_file_matches(path: &Path, expected_size: u64, expected_sha256: &str) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_file() => Ok(false),
        Ok(_) => file_matches(path, expected_size, expected_sha256),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn unique_attachment_conflict_path(destination: &Path) -> Result<PathBuf> {
    let parent = destination
        .parent()
        .ok_or(Error::LocalAttachmentUnavailable)?;
    let filename = destination
        .file_name()
        .ok_or(Error::LocalAttachmentUnavailable)?
        .to_string_lossy();
    Ok(parent.join(format!("{filename}.anarlog-conflict-{}", Uuid::new_v4())))
}

fn private_cache_root<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(|_| Error::CacheUnavailable)?
        .join("attachment-sync")
        .join("private"))
}

fn private_cache_path(root: &Path, cache_id: &str) -> Result<PathBuf> {
    if !valid_cache_id(cache_id) {
        return Err(Error::InvalidTransferState);
    }
    Ok(root.join(format!("{cache_id}.anb1")))
}

fn shared_upload_cache_root<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(|_| Error::CacheUnavailable)?
        .join("attachment-sync")
        .join("shared-upload"))
}

async fn create_shared_upload_cache_root(path: &Path) -> Result<()> {
    tokio::fs::create_dir_all(path).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await?;
    }
    Ok(())
}

fn shared_upload_cache_path(root: &Path, cache_id: &str) -> Result<PathBuf> {
    if !valid_cache_id(cache_id) {
        return Err(Error::InvalidTransferState);
    }
    Ok(root.join(format!("{cache_id}.bin")))
}

fn delete_guard_root<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|_| Error::CacheUnavailable)?
        .join("attachment-sync")
        .join("delete-guards"))
}

async fn create_delete_guard_root(path: &Path) -> Result<()> {
    tokio::fs::create_dir_all(path).await?;
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::CacheUnavailable);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await?;
    }
    Ok(())
}

fn delete_guard_path(root: &Path, guard_id: &str) -> Result<PathBuf> {
    if !valid_cache_id(guard_id) {
        return Err(Error::InvalidTransferState);
    }
    Ok(root.join(format!("{guard_id}.anb1")))
}

async fn guarded_file_matches_async(
    path: PathBuf,
    expected_size: u64,
    expected_sha256: String,
    cancellation: tokio_util::sync::CancellationToken,
) -> Result<bool> {
    tokio::task::spawn_blocking(move || {
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if !metadata.file_type().is_file() {
            return Ok(false);
        }
        file_matches_cancellable(&path, expected_size, &expected_sha256, &cancellation)
    })
    .await
    .map_err(|_| Error::CacheUnavailable)?
}

async fn cleanup_linked_delete_guard<R: Runtime>(app: &tauri::AppHandle<R>, guard_id: &str) {
    if !valid_cache_id(guard_id) {
        return;
    }
    let Ok(path) = delete_guard_path(
        &match delete_guard_root(app) {
            Ok(root) => root,
            Err(_) => return,
        },
        guard_id,
    ) else {
        return;
    };
    let _ = tokio::task::spawn_blocking(move || cleanup_delete_guard_path(&path)).await;
}

fn cleanup_delete_guard_path(path: &Path) -> Result<bool> {
    cleanup_shared_upload_path(path)
}

fn reconcile_delete_guard_files(
    root: &Path,
    referenced: &HashSet<String>,
    orphan_before: SystemTime,
) -> Result<u64> {
    let root_metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    if !root_metadata.file_type().is_dir() || root_metadata.file_type().is_symlink() {
        return Err(Error::CacheUnavailable);
    }
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let mut removed = 0_u64;
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_file() && !file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "anb1") {
            continue;
        }
        let guard_id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("");
        if valid_cache_id(guard_id) && referenced.contains(guard_id) {
            continue;
        }
        if std::fs::symlink_metadata(&path)?.modified()? > orphan_before {
            continue;
        }
        if cleanup_delete_guard_path(&path)? {
            removed = removed.saturating_add(1);
        }
    }
    Ok(removed)
}

fn cleanup_shared_upload_path(path: &Path) -> Result<bool> {
    let mut last_error = None;
    for delay_ms in [0, 50, 250, 1_000] {
        if delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
        match std::fs::remove_file(path) {
            Ok(()) => return Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.ok_or(Error::CacheUnavailable)?.into())
}

fn shared_scope_path<R: Runtime>(app: &tauri::AppHandle<R>, scope_id: &str) -> Result<PathBuf> {
    Ok(shared_cache_root(app)?.join(hash_identifier(scope_id)))
}

fn shared_cache_root<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(|_| Error::CacheUnavailable)?
        .join("attachment-sync")
        .join("shared"))
}

fn clear_attachment_cache_directory(path: &Path) -> Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn cached_shared_attachment_path(scope_path: &Path, cache_id: &str) -> PathBuf {
    scope_path.join(format!("{cache_id}.bin"))
}

fn shared_cache_metadata_path(scope_path: &Path, cache_id: &str) -> PathBuf {
    scope_path.join(format!("{cache_id}.meta"))
}

fn write_shared_cache_metadata(
    scope_path: &Path,
    cache_id: &str,
    size_bytes: u64,
    sha256: &str,
) -> Result<()> {
    let path = shared_cache_metadata_path(scope_path, cache_id);
    let mut temp = tempfile::NamedTempFile::new_in(scope_path)?;
    write!(temp, "{size_bytes}\n{sha256}\n")?;
    temp.flush()?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| Error::Io(error.error))?;
    Ok(())
}

fn commit_shared_cache_entry(
    scope_path: &Path,
    cache_id: &str,
    data_path: &Path,
    size_bytes: u64,
    sha256: &str,
) -> Result<()> {
    let data_guard = CacheFileGuard::new(data_path.to_path_buf());
    write_shared_cache_metadata(scope_path, cache_id, size_bytes, sha256)?;
    data_guard.disarm();
    Ok(())
}

fn read_shared_cache_metadata(scope_path: &Path, cache_id: &str) -> Result<Option<(u64, String)>> {
    let path = shared_cache_metadata_path(scope_path, cache_id);
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() > 128 {
        return Ok(None);
    }
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut lines = content.lines();
    let size = lines
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value <= MAX_PLAINTEXT_BYTES);
    let sha256 = lines.next().filter(|value| valid_sha256(value));
    if lines.next().is_some() || size.is_none() || sha256.is_none() {
        return Ok(None);
    }
    Ok(Some((size.unwrap(), sha256.unwrap().to_string())))
}

fn shared_cache_matches(
    scope_path: &Path,
    cache_id: &str,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<bool> {
    let Some((recorded_size, recorded_sha256)) = read_shared_cache_metadata(scope_path, cache_id)?
    else {
        return Ok(false);
    };
    if recorded_size != expected_size || recorded_sha256 != expected_sha256 {
        return Ok(false);
    }
    file_matches(
        &cached_shared_attachment_path(scope_path, cache_id),
        expected_size,
        expected_sha256,
    )
}

async fn shared_cache_matches_async(
    scope_path: PathBuf,
    cache_id: String,
    expected_size: u64,
    expected_sha256: String,
) -> Result<bool> {
    tokio::task::spawn_blocking(move || {
        shared_cache_matches(&scope_path, &cache_id, expected_size, &expected_sha256)
    })
    .await
    .map_err(|_| Error::CacheUnavailable)?
}

fn hash_identifier(value: &str) -> String {
    hex_digest(Sha256::digest(value.as_bytes()).as_slice())
}

fn shared_cache_id(scope_id: &str, attachment_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update((scope_id.len() as u64).to_be_bytes());
    hasher.update(scope_id.as_bytes());
    hasher.update((attachment_id.len() as u64).to_be_bytes());
    hasher.update(attachment_id.as_bytes());
    hex_digest(hasher.finalize().as_slice())
}

fn hex_digest(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut value, byte| {
            write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
            value
        })
}

enum DownloadObject<'a> {
    Private(&'a str),
    Shared(&'a str),
}

fn require_configured_supabase_url(value: Option<&str>) -> Result<&str> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or(Error::InvalidDownloadUrl)
}

fn validate_signed_download_url(
    signed_url: &str,
    supabase_url: &str,
    object: DownloadObject<'_>,
) -> Result<url::Url> {
    let base = url::Url::parse(supabase_url).map_err(|_| Error::InvalidDownloadUrl)?;
    let signed = url::Url::parse(signed_url).map_err(|_| Error::InvalidDownloadUrl)?;
    if !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
        || !matches!(base.path(), "" | "/")
        || !signed.username().is_empty()
        || signed.password().is_some()
        || signed.fragment().is_some()
        || base.scheme() != signed.scheme()
        || base.host_str() != signed.host_str()
        || base.port_or_known_default() != signed.port_or_known_default()
        || !secure_or_local(&base)
    {
        return Err(Error::InvalidDownloadUrl);
    }
    let mut query = signed.query_pairs();
    if !query
        .next()
        .is_some_and(|(name, value)| name == "token" && !value.is_empty())
        || query.next().is_some()
    {
        return Err(Error::InvalidDownloadUrl);
    }
    let valid_path = match object {
        DownloadObject::Private(object_key) => {
            signed.path() == format!("/storage/v1/object/sign/attachment-backups/{object_key}")
        }
        DownloadObject::Shared(attachment_id) => {
            valid_shared_attachment_download_path(signed.path(), attachment_id)
        }
    };
    if !valid_path {
        return Err(Error::InvalidDownloadUrl);
    }
    Ok(signed)
}

fn valid_shared_attachment_download_path(path: &str, attachment_id: &str) -> bool {
    let Some(object_key) = path.strip_prefix("/storage/v1/object/sign/shared-note-attachments/")
    else {
        return false;
    };
    let mut parts = object_key.split('/');
    let owner = parts.next();
    let share = parts.next();
    let filename = parts.next();
    parts.next().is_none()
        && owner.is_some_and(valid_canonical_uuid)
        && share.is_some_and(valid_canonical_uuid)
        && filename.is_some_and(|filename| filename == format!("{attachment_id}.sna1"))
}

fn valid_canonical_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|uuid| uuid.to_string() == value)
}

fn secure_or_local(url: &url::Url) -> bool {
    if url.scheme() == "https" {
        return true;
    }
    if url.scheme() != "http" {
        return false;
    }
    match url.host() {
        Some(url::Host::Domain("localhost")) => true,
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        _ => false,
    }
}

async fn download_to_path(
    url: url::Url,
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
    create_new: bool,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(Error::Download)?;
    let response = tokio::select! {
        _ = cancellation.cancelled() => return Err(Error::Cancelled),
        response = client
            .get(url)
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .send() => response.map_err(Error::Download)?,
    };
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|size| size != expected_size)
        || response
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .is_some_and(|value| value != "identity")
    {
        return Err(Error::IncompleteDownload);
    }

    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).truncate(!create_new);
    if create_new {
        options.create_new(true);
    }
    let mut file = options.open(path).await?;
    let mut stream = response.bytes_stream();
    let mut size = 0_u64;
    let mut hasher = Sha256::new();
    loop {
        let next = tokio::select! {
            _ = cancellation.cancelled() => return Err(Error::Cancelled),
            next = stream.next() => next,
        };
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk.map_err(Error::Download)?;
        size = size
            .checked_add(chunk.len() as u64)
            .ok_or(Error::IncompleteDownload)?;
        if size > expected_size {
            return Err(Error::IncompleteDownload);
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    tokio::select! {
        _ = cancellation.cancelled() => return Err(Error::Cancelled),
        result = file.sync_all() => result?,
    }
    if size != expected_size || hex_digest(hasher.finalize().as_slice()) != expected_sha256 {
        return Err(Error::ChecksumMismatch);
    }
    Ok(())
}

fn stage_attachment_restore(
    key: &WorkspaceKey,
    context: &AttachmentBlobContext,
    cache_path: &Path,
    destination_parent: &Path,
    expected: &AttachmentBlobMetadata,
) -> Result<tempfile::NamedTempFile> {
    std::fs::create_dir_all(destination_parent)?;
    let mut source = std::fs::File::open(cache_path)?;
    let mut temp = tempfile::NamedTempFile::new_in(destination_parent)?;
    key.open_attachment_blob(context, &mut source, &mut temp, expected)?;
    temp.flush()?;
    temp.as_file().sync_all()?;
    Ok(temp)
}

fn stage_delete_guard_restore(
    key: &WorkspaceKey,
    context: &AttachmentBlobContext,
    guard_path: &Path,
    destination_parent: &Path,
    expected: &AttachmentBlobMetadata,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<tempfile::NamedTempFile> {
    std::fs::create_dir_all(destination_parent)?;
    let mut source = CancellableReader {
        inner: std::fs::File::open(guard_path)?,
        cancellation: cancellation.clone(),
    };
    let mut temp = tempfile::NamedTempFile::new_in(destination_parent)?;
    match key.open_attachment_blob(context, &mut source, &mut temp, expected) {
        Ok(_) => {}
        Err(hypr_e2ee::AttachmentBlobError::Io(error))
            if error.kind() == std::io::ErrorKind::ConnectionAborted =>
        {
            return Err(Error::Cancelled);
        }
        Err(error) => return Err(error.into()),
    }
    temp.flush()?;
    temp.as_file().sync_all()?;
    Ok(temp)
}

fn persist_staged_attachment(staged: tempfile::NamedTempFile, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .ok_or(Error::LocalAttachmentUnavailable)?;
    staged
        .persist(destination)
        .map(|_| ())
        .map_err(|error| Error::Io(error.error))?;
    sync_destination_directory(parent)
}

#[cfg(unix)]
fn sync_destination_directory(path: &Path) -> Result<()> {
    std::fs::File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_destination_directory(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transfer_record(cloud_sync_enabled: i64) -> TransferAttachment {
        TransferAttachment {
            job_id: "job-1".to_string(),
            attachment_id: "attachment-1".to_string(),
            session_id: "session-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            expected_sha256: "a".repeat(64),
            expected_size_bytes: 1,
            ciphertext_sha256: String::new(),
            ciphertext_size_bytes: 0,
            remote_object_id: String::new(),
            object_key: String::new(),
            cache_id: String::new(),
            phase: "preparing".to_string(),
            relative_path: "attachments/file.bin".to_string(),
            source_type: "note_upload".to_string(),
            attachment_sha256: "a".repeat(64),
            attachment_size_bytes: 1,
            attachment_cloud_object_key: String::new(),
            cloud_sync_enabled,
        }
    }

    fn delete_source_record(cloud_sync_enabled: i64) -> DeleteSourcePreflight {
        DeleteSourcePreflight {
            attachment_id: "attachment-1".to_string(),
            session_id: "session-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            expected_sha256: "a".repeat(64),
            expected_size_bytes: 1,
            object_key: "owner/object.anb1".to_string(),
            cache_id: String::new(),
            ciphertext_sha256: String::new(),
            ciphertext_size_bytes: 0,
            current_attachment_id: Some("attachment-1".to_string()),
            relative_path: Some("attachments/file.bin".to_string()),
            source_type: Some("note_upload".to_string()),
            attachment_sha256: Some("a".repeat(64)),
            attachment_size_bytes: Some(1),
            attachment_cloud_object_key: Some("owner/object.anb1".to_string()),
            cloud_sync_enabled: Some(cloud_sync_enabled),
            deleted_at: None,
        }
    }

    #[test]
    fn disabled_cloud_sync_is_restore_only() {
        let disabled = transfer_record(0);
        assert!(validate_transfer_version(&disabled).is_ok());
        assert!(validate_upload_transfer_version(&disabled).is_err());

        let enabled = transfer_record(1);
        assert!(validate_transfer_version(&enabled).is_ok());
        assert!(validate_upload_transfer_version(&enabled).is_ok());
    }

    #[test]
    fn only_an_exact_current_delete_source_requires_hashing() {
        assert!(
            delete_source_attachment(&delete_source_record(0))
                .unwrap()
                .is_some()
        );
        assert!(
            delete_source_attachment(&delete_source_record(1))
                .unwrap()
                .is_some()
        );

        let mut missing = delete_source_record(0);
        missing.current_attachment_id = None;
        assert!(delete_source_attachment(&missing).unwrap().is_none());

        let mut deleted = delete_source_record(0);
        deleted.deleted_at = Some("2026-07-18T00:00:00.000Z".to_string());
        deleted.relative_path = None;
        assert!(delete_source_attachment(&deleted).unwrap().is_none());

        let mut replaced = delete_source_record(0);
        replaced.attachment_sha256 = Some("b".repeat(64));
        assert!(delete_source_attachment(&replaced).unwrap().is_none());

        let mut other_object = delete_source_record(0);
        other_object.attachment_cloud_object_key = Some("owner/other.anb1".to_string());
        assert!(delete_source_attachment(&other_object).unwrap().is_none());
    }

    #[test]
    fn delete_source_hash_detects_missing_and_changed_local_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("attachment.bin");
        let bytes = b"preserved attachment";
        let sha256 = hex_digest(Sha256::digest(bytes).as_slice());

        assert!(!file_matches(&path, bytes.len() as u64, &sha256).unwrap());
        std::fs::write(&path, bytes).unwrap();
        assert!(file_matches(&path, bytes.len() as u64, &sha256).unwrap());
        std::fs::write(&path, b"different attachment").unwrap();
        assert!(!file_matches(&path, bytes.len() as u64, &sha256).unwrap());
    }

    #[test]
    fn shared_upload_snapshot_stays_stable_when_the_source_changes() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("attachment.bin");
        let snapshot = directory.path().join("snapshot.bin");
        let bytes = b"stable shared attachment";
        let sha256 = hex_digest(Sha256::digest(bytes).as_slice());
        std::fs::write(&source, bytes).unwrap();

        snapshot_verified_file(
            &source,
            &snapshot,
            bytes.len() as u64,
            &sha256,
            &tokio_util::sync::CancellationToken::new(),
        )
        .unwrap()
        .disarm();
        std::fs::write(&source, vec![b'x'; bytes.len()]).unwrap();

        assert_eq!(std::fs::read(&snapshot).unwrap(), bytes);
        assert!(file_matches(&snapshot, bytes.len() as u64, &sha256).unwrap());
        assert!(!file_matches(&source, bytes.len() as u64, &sha256).unwrap());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&snapshot).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn shared_upload_snapshot_rejects_and_removes_unregistered_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("attachment.bin");
        let snapshot = directory.path().join("snapshot.bin");
        let expected = b"expected shared attachment";
        let sha256 = hex_digest(Sha256::digest(expected).as_slice());
        std::fs::write(&source, vec![b'x'; expected.len()]).unwrap();

        assert!(matches!(
            snapshot_verified_file(
                &source,
                &snapshot,
                expected.len() as u64,
                &sha256,
                &tokio_util::sync::CancellationToken::new(),
            ),
            Err(Error::ChecksumMismatch)
        ));
        assert!(!snapshot.exists());
    }

    #[test]
    fn cancelled_shared_upload_snapshot_leaves_no_plaintext_cache() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("attachment.bin");
        let snapshot = directory.path().join("snapshot.bin");
        let bytes = b"cancelled shared attachment";
        let sha256 = hex_digest(Sha256::digest(bytes).as_slice());
        let cancellation = tokio_util::sync::CancellationToken::new();
        cancellation.cancel();
        std::fs::write(&source, bytes).unwrap();

        assert!(matches!(
            snapshot_verified_file(
                &source,
                &snapshot,
                bytes.len() as u64,
                &sha256,
                &cancellation,
            ),
            Err(Error::Cancelled)
        ));
        assert!(!snapshot.exists());
    }

    #[test]
    fn shared_upload_cleanup_is_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let snapshot = directory.path().join("snapshot.bin");
        std::fs::write(&snapshot, b"shared attachment").unwrap();

        assert!(cleanup_shared_upload_path(&snapshot).unwrap());
        assert!(!cleanup_shared_upload_path(&snapshot).unwrap());
    }

    #[tokio::test]
    async fn shared_upload_cache_directory_is_owner_only() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("shared-upload");

        create_shared_upload_cache_root(&root).await.unwrap();
        assert!(root.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(root).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
    }

    #[test]
    fn validates_upload_ranges() {
        assert!(validate_range(0, 1).is_ok());
        assert!(validate_range(0, MAX_RANGE_BYTES).is_ok());
        assert!(validate_range(0, 0).is_err());
        assert!(validate_range(2, 1).is_err());
        assert!(validate_range(0, MAX_RANGE_BYTES + 1).is_err());
    }

    #[test]
    fn only_accepts_expected_attachment_paths() {
        assert!(validate_attachment_relative_path("note_upload", "attachments/image.png").is_ok());
        assert!(validate_attachment_relative_path("session_audio", "audio.wav").is_ok());
        assert!(
            validate_attachment_relative_path("session_audio", "attachments/audio.wav").is_err()
        );
        assert!(validate_attachment_relative_path("note_upload", "../image.png").is_err());
        assert!(validate_attachment_relative_path("note_upload", "attachments/a/b.png").is_err());
    }

    #[test]
    fn signed_urls_are_origin_and_path_bound() {
        let object_key =
            "00000000-0000-4000-8000-000000000001/00000000-0000-4000-8000-000000000002.anb1";
        let valid = format!(
            "https://project.supabase.co/storage/v1/object/sign/attachment-backups/{object_key}?token=secret"
        );
        assert!(
            validate_signed_download_url(
                &valid,
                "https://project.supabase.co",
                DownloadObject::Private(object_key),
            )
            .is_ok()
        );
        assert!(
            validate_signed_download_url(
                &valid,
                "https://other.supabase.co",
                DownloadObject::Private(object_key),
            )
            .is_err()
        );
        assert!(
            validate_signed_download_url(
                "http://project.supabase.co/storage/v1/object/sign/bucket/item?token=secret",
                "http://project.supabase.co",
                DownloadObject::Private(object_key),
            )
            .is_err()
        );
        assert!(
            validate_signed_download_url(
                &format!(
                    "http://127.0.0.1:54321/storage/v1/object/sign/attachment-backups/{object_key}?token=secret"
                ),
                "http://127.0.0.1:54321",
                DownloadObject::Private(object_key),
            )
            .is_ok()
        );

        let attachment_id = "00000000-0000-4000-8000-000000000003";
        let shared = format!(
            "https://project.supabase.co/storage/v1/object/sign/shared-note-attachments/00000000-0000-4000-8000-000000000001/00000000-0000-4000-8000-000000000002/{attachment_id}.sna1?token=secret"
        );
        assert!(
            validate_signed_download_url(
                &shared,
                "https://project.supabase.co",
                DownloadObject::Shared(attachment_id),
            )
            .is_ok()
        );
        assert!(
            validate_signed_download_url(
                &shared.replace("shared-note-attachments", "other-bucket"),
                "https://project.supabase.co",
                DownloadObject::Shared(attachment_id),
            )
            .is_err()
        );
        assert!(
            validate_signed_download_url(
                &shared.replace(attachment_id, "00000000-0000-4000-8000-000000000004"),
                "https://project.supabase.co",
                DownloadObject::Shared(attachment_id),
            )
            .is_err()
        );
    }

    #[test]
    fn missing_or_empty_configured_origins_fail_closed() {
        assert!(require_configured_supabase_url(None).is_err());
        assert!(require_configured_supabase_url(Some("")).is_err());
        assert!(require_configured_supabase_url(Some("   ")).is_err());
        assert_eq!(
            require_configured_supabase_url(Some("https://project.supabase.co")).unwrap(),
            "https://project.supabase.co"
        );
    }

    #[test]
    fn scoped_cache_names_do_not_expose_ids() {
        let first = hash_identifier("share/../../secret");
        let second = hash_identifier("share/../../secret");
        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(!first.contains("secret"));
        assert_ne!(
            shared_cache_id("viewer-a", "attachment-a"),
            shared_cache_id("viewer-b", "attachment-a")
        );
    }

    #[test]
    fn restores_encrypted_attachment_atomically() {
        let directory = tempfile::tempdir().unwrap();
        let source_path = directory.path().join("source.bin");
        let cache_path = directory.path().join("cache.anb1");
        let destination_path = directory.path().join("destination.bin");
        let plaintext = b"private attachment bytes";
        std::fs::write(&source_path, plaintext).unwrap();
        std::fs::write(&destination_path, b"old bytes").unwrap();

        let key = hypr_e2ee::RecoveryKey::generate()
            .unwrap()
            .workspace_key("workspace-a")
            .unwrap();
        let context =
            AttachmentBlobContext::new("workspace-a", "attachment-a", Uuid::new_v4().to_string())
                .unwrap();
        let expected_plaintext = AttachmentBlobPlaintextMetadata::from_hex(
            plaintext.len() as u64,
            &hex_digest(Sha256::digest(plaintext).as_slice()),
        )
        .unwrap();
        let metadata = {
            let mut source = std::fs::File::open(source_path).unwrap();
            let mut cache = std::fs::File::create(&cache_path).unwrap();
            key.seal_attachment_blob(&context, &mut source, &mut cache, &expected_plaintext)
                .unwrap()
        };

        let staged = stage_attachment_restore(
            &key,
            &context,
            &cache_path,
            destination_path.parent().unwrap(),
            &metadata,
        )
        .unwrap();
        assert_eq!(std::fs::read(&destination_path).unwrap(), b"old bytes");
        persist_staged_attachment(staged, &destination_path).unwrap();
        assert_eq!(std::fs::read(destination_path).unwrap(), plaintext);
    }

    #[test]
    fn delete_guard_restores_canonical_bytes_and_preserves_a_conflict() {
        let directory = tempfile::tempdir().unwrap();
        let source_path = directory.path().join("source.bin");
        let guard_path = directory.path().join(format!("{}.anb1", Uuid::new_v4()));
        let destination_path = directory.path().join("destination.bin");
        let canonical = b"canonical private attachment";
        let local_edit = b"different local attachment";
        std::fs::write(&source_path, canonical).unwrap();

        let key = hypr_e2ee::RecoveryKey::generate()
            .unwrap()
            .workspace_key("workspace-a")
            .unwrap();
        let context =
            AttachmentBlobContext::new("workspace-a", "attachment-a", Uuid::new_v4().to_string())
                .unwrap();
        let expected = AttachmentBlobPlaintextMetadata::from_hex(
            canonical.len() as u64,
            &hex_digest(Sha256::digest(canonical).as_slice()),
        )
        .unwrap();
        let (metadata, guard) = seal_delete_guard(
            &key,
            &context,
            &source_path,
            &guard_path,
            &expected,
            &tokio_util::sync::CancellationToken::new(),
        )
        .unwrap();
        guard.disarm();

        assert_ne!(std::fs::read(&guard_path).unwrap(), canonical);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&guard_path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        std::fs::write(&source_path, b"source changed after delete began").unwrap();
        std::fs::write(&destination_path, local_edit).unwrap();
        let staged = stage_delete_guard_restore(
            &key,
            &context,
            &guard_path,
            directory.path(),
            &metadata,
            &tokio_util::sync::CancellationToken::new(),
        )
        .unwrap();
        let conflicts = reconcile_staged_delete_guard(
            staged,
            &destination_path,
            canonical.len() as u64,
            &expected.sha256_hex(),
        )
        .unwrap();

        assert_eq!(std::fs::read(&destination_path).unwrap(), canonical);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(std::fs::read(&conflicts[0]).unwrap(), local_edit);

        let staged = stage_delete_guard_restore(
            &key,
            &context,
            &guard_path,
            directory.path(),
            &metadata,
            &tokio_util::sync::CancellationToken::new(),
        )
        .unwrap();
        assert!(
            reconcile_staged_delete_guard(
                staged,
                &destination_path,
                canonical.len() as u64,
                &expected.sha256_hex(),
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn delete_guard_restores_a_missing_destination() {
        let directory = tempfile::tempdir().unwrap();
        let source_path = directory.path().join("source.bin");
        let guard_path = directory.path().join(format!("{}.anb1", Uuid::new_v4()));
        let destination_path = directory.path().join("missing.bin");
        let canonical = b"attachment recovered after remote delete";
        std::fs::write(&source_path, canonical).unwrap();
        let key = hypr_e2ee::RecoveryKey::generate()
            .unwrap()
            .workspace_key("workspace-a")
            .unwrap();
        let context =
            AttachmentBlobContext::new("workspace-a", "attachment-a", Uuid::new_v4().to_string())
                .unwrap();
        let expected = AttachmentBlobPlaintextMetadata::from_hex(
            canonical.len() as u64,
            &hex_digest(Sha256::digest(canonical).as_slice()),
        )
        .unwrap();
        let (metadata, guard) = seal_delete_guard(
            &key,
            &context,
            &source_path,
            &guard_path,
            &expected,
            &tokio_util::sync::CancellationToken::new(),
        )
        .unwrap();
        guard.disarm();
        let staged = stage_delete_guard_restore(
            &key,
            &context,
            &guard_path,
            directory.path(),
            &metadata,
            &tokio_util::sync::CancellationToken::new(),
        )
        .unwrap();

        assert!(
            reconcile_staged_delete_guard(
                staged,
                &destination_path,
                canonical.len() as u64,
                &expected.sha256_hex(),
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(std::fs::read(destination_path).unwrap(), canonical);
    }

    #[test]
    fn delete_guard_retry_keeps_the_plaintext_stage() {
        let directory = tempfile::tempdir().unwrap();
        let blocked_parent = directory.path().join("blocked-parent");
        let destination = blocked_parent.join("attachment.bin");
        let canonical = b"attachment retained across filesystem retry";
        let expected_sha256 = hex_digest(Sha256::digest(canonical).as_slice());
        let staged = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
        std::fs::write(staged.path(), canonical).unwrap();
        std::fs::write(&blocked_parent, b"not a directory").unwrap();

        let (staged, conflicts) = match reconcile_staged_delete_guard_once(
            staged,
            &destination,
            canonical.len() as u64,
            &expected_sha256,
            Vec::new(),
        )
        .unwrap()
        {
            DeleteGuardReconcile::Retry {
                staged,
                conflicts,
                error: Error::Io(_),
            } => (staged, conflicts),
            _ => panic!("blocked parent should produce a retryable filesystem error"),
        };
        assert!(staged.path().exists());

        std::fs::remove_file(blocked_parent).unwrap();
        let conflicts = match reconcile_staged_delete_guard_once(
            staged,
            &destination,
            canonical.len() as u64,
            &expected_sha256,
            conflicts,
        )
        .unwrap()
        {
            DeleteGuardReconcile::Ready(conflicts) => conflicts,
            DeleteGuardReconcile::Retry { .. } => panic!("retry should restore the attachment"),
        };

        assert!(conflicts.is_empty());
        assert_eq!(std::fs::read(destination).unwrap(), canonical);
    }

    #[test]
    fn cancelled_delete_guard_seal_removes_partial_ciphertext() {
        let directory = tempfile::tempdir().unwrap();
        let source_path = directory.path().join("source.bin");
        let guard_path = directory.path().join(format!("{}.anb1", Uuid::new_v4()));
        let canonical = b"attachment whose delete was cancelled";
        std::fs::write(&source_path, canonical).unwrap();
        let key = hypr_e2ee::RecoveryKey::generate()
            .unwrap()
            .workspace_key("workspace-a")
            .unwrap();
        let context =
            AttachmentBlobContext::new("workspace-a", "attachment-a", Uuid::new_v4().to_string())
                .unwrap();
        let expected = AttachmentBlobPlaintextMetadata::from_hex(
            canonical.len() as u64,
            &hex_digest(Sha256::digest(canonical).as_slice()),
        )
        .unwrap();
        let cancellation = tokio_util::sync::CancellationToken::new();
        cancellation.cancel();

        assert!(matches!(
            seal_delete_guard(
                &key,
                &context,
                &source_path,
                &guard_path,
                &expected,
                &cancellation,
            ),
            Err(Error::Cancelled)
        ));
        assert!(!guard_path.exists());
    }

    #[test]
    fn cancelled_delete_guard_restore_removes_plaintext_stage() {
        let directory = tempfile::tempdir().unwrap();
        let source_path = directory.path().join("source.bin");
        let guard_path = directory.path().join(format!("{}.anb1", Uuid::new_v4()));
        let canonical = b"attachment whose restore was cancelled";
        std::fs::write(&source_path, canonical).unwrap();
        let key = hypr_e2ee::RecoveryKey::generate()
            .unwrap()
            .workspace_key("workspace-a")
            .unwrap();
        let context =
            AttachmentBlobContext::new("workspace-a", "attachment-a", Uuid::new_v4().to_string())
                .unwrap();
        let expected = AttachmentBlobPlaintextMetadata::from_hex(
            canonical.len() as u64,
            &hex_digest(Sha256::digest(canonical).as_slice()),
        )
        .unwrap();
        let (metadata, guard) = seal_delete_guard(
            &key,
            &context,
            &source_path,
            &guard_path,
            &expected,
            &tokio_util::sync::CancellationToken::new(),
        )
        .unwrap();
        guard.disarm();
        let cancellation = tokio_util::sync::CancellationToken::new();
        cancellation.cancel();

        assert!(matches!(
            stage_delete_guard_restore(
                &key,
                &context,
                &guard_path,
                directory.path(),
                &metadata,
                &cancellation,
            ),
            Err(Error::Cancelled)
        ));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn recoverable_delete_guard_drift_uses_a_retryable_message() {
        let message = Error::DeleteGuardChanged.to_string().to_ascii_lowercase();
        for permanent_marker in ["invalid", "mismatch", "path", "source"] {
            assert!(!message.contains(permanent_marker));
        }
    }

    #[tokio::test]
    async fn delete_guard_directory_is_owner_only() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("delete-guards");

        create_delete_guard_root(&root).await.unwrap();
        assert!(root.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(root).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
    }

    #[test]
    fn delete_guard_reconciliation_retains_only_live_references() {
        let directory = tempfile::tempdir().unwrap();
        let live = Uuid::new_v4().to_string();
        let orphan = Uuid::new_v4().to_string();
        std::fs::write(directory.path().join(format!("{live}.anb1")), b"live").unwrap();
        std::fs::write(directory.path().join(format!("{orphan}.anb1")), b"orphan").unwrap();
        std::fs::write(directory.path().join("malformed.anb1"), b"malformed").unwrap();

        assert_eq!(
            reconcile_delete_guard_files(
                directory.path(),
                &HashSet::from([live.clone()]),
                SystemTime::now() + Duration::from_secs(1),
            )
            .unwrap(),
            2
        );
        assert!(directory.path().join(format!("{live}.anb1")).is_file());
        assert!(!directory.path().join(format!("{orphan}.anb1")).exists());
        assert!(!directory.path().join("malformed.anb1").exists());
    }

    #[test]
    fn delete_guard_reconciliation_does_not_race_a_new_guard() {
        let directory = tempfile::tempdir().unwrap();
        let guard_id = Uuid::new_v4().to_string();
        let guard_path = directory.path().join(format!("{guard_id}.anb1"));
        std::fs::write(&guard_path, b"new unlinked guard").unwrap();

        assert_eq!(
            reconcile_delete_guard_files(
                directory.path(),
                &HashSet::new(),
                SystemTime::now() - DELETE_GUARD_ORPHAN_GRACE,
            )
            .unwrap(),
            0
        );
        assert!(guard_path.is_file());
    }

    #[test]
    fn shared_cache_requires_matching_sidecar_and_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let cache_id = shared_cache_id("viewer-a", "attachment-a");
        let bytes = b"shared attachment";
        let sha256 = hex_digest(Sha256::digest(bytes).as_slice());
        std::fs::write(
            cached_shared_attachment_path(directory.path(), &cache_id),
            bytes,
        )
        .unwrap();
        write_shared_cache_metadata(directory.path(), &cache_id, bytes.len() as u64, &sha256)
            .unwrap();
        assert!(
            shared_cache_matches(directory.path(), &cache_id, bytes.len() as u64, &sha256).unwrap()
        );

        std::fs::write(
            cached_shared_attachment_path(directory.path(), &cache_id),
            b"tampered attachment",
        )
        .unwrap();
        assert!(
            !shared_cache_matches(directory.path(), &cache_id, bytes.len() as u64, &sha256)
                .unwrap()
        );
    }

    #[test]
    fn shared_cache_removes_plaintext_when_sidecar_commit_fails() {
        let directory = tempfile::tempdir().unwrap();
        let cache_id = shared_cache_id("viewer-a", "attachment-a");
        let data_path = cached_shared_attachment_path(directory.path(), &cache_id);
        std::fs::write(&data_path, b"shared attachment").unwrap();
        std::fs::create_dir(shared_cache_metadata_path(directory.path(), &cache_id)).unwrap();

        assert!(
            commit_shared_cache_entry(
                directory.path(),
                &cache_id,
                &data_path,
                17,
                &"a".repeat(64),
            )
            .is_err()
        );
        assert!(!data_path.exists());
    }

    #[test]
    fn startup_cleanup_removes_an_entire_cache_root() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("shared");
        let orphan = root.join("orphaned-viewer");
        std::fs::create_dir_all(&orphan).unwrap();
        std::fs::write(orphan.join("attachment.bin"), b"plaintext").unwrap();

        clear_attachment_cache_directory(&root).unwrap();

        assert!(!root.exists());
        clear_attachment_cache_directory(&root).unwrap();
    }

    #[test]
    fn cache_file_guard_removes_abandoned_files_only() {
        let directory = tempfile::tempdir().unwrap();
        let abandoned = directory.path().join("abandoned.anb1");
        std::fs::write(&abandoned, b"partial").unwrap();
        drop(CacheFileGuard::new(abandoned.clone()));
        assert!(!abandoned.exists());

        let retained = directory.path().join("retained.anb1");
        std::fs::write(&retained, b"complete").unwrap();
        CacheFileGuard::new(retained.clone()).disarm();
        assert!(retained.exists());
    }
}

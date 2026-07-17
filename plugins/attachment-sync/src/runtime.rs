use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

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
    PreparedUpload, RestoredAttachment, SharedAttachmentCacheResult, UploadDescriptor,
};

const FORMAT_VERSION: i16 = 1;
const MAX_RANGE_BYTES: u64 = 6 * 1024 * 1024;
const MAX_PLAINTEXT_BYTES: u64 = hypr_e2ee::ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES;
const MAX_CIPHERTEXT_BYTES: u64 = 545_259_520;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

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

#[derive(Debug, Clone, FromRow)]
struct LocalAttachment {
    attachment_id: String,
    session_id: String,
    workspace_id: String,
    relative_path: String,
    source_type: String,
    sha256: String,
    size_bytes: i64,
}

#[derive(Debug, Clone, FromRow)]
struct DeleteSourcePreflight {
    session_id: String,
    workspace_id: String,
    expected_sha256: String,
    expected_size_bytes: i64,
    object_key: String,
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

pub async fn read_attachment_range<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri_plugin_db::ManagedState,
    attachment_id: &str,
    start: u64,
    end: u64,
) -> Result<Vec<u8>> {
    validate_range(start, end)?;
    let attachment = load_local_attachment(state.pool(), attachment_id).await?;
    let size = valid_plaintext_size(attachment.size_bytes)?;
    if end > size {
        return Err(Error::InvalidRange);
    }
    let path = resolve_attachment_path(app, &attachment, true)?;
    read_range(path, start, end, size).await
}

pub async fn verify_delete_source<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri_plugin_db::ManagedState,
    job_id: &str,
    attempt_count: i64,
) -> Result<bool> {
    validate_opaque_id(job_id)?;
    if attempt_count <= 0 {
        return Err(Error::InvalidTransferState);
    }
    let record = sqlx::query_as::<_, DeleteSourcePreflight>(
        "SELECT
           job.session_id,
           job.workspace_id,
           job.expected_sha256,
           job.expected_size_bytes,
           job.object_key,
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
         LIMIT 1",
    )
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
        return Ok(true);
    };
    let (_, path) = attachment_paths(app, &attachment)?;
    file_matches_async(path, expected_size, record.expected_sha256).await
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

async fn load_local_attachment(
    pool: &sqlx::SqlitePool,
    attachment_id: &str,
) -> Result<LocalAttachment> {
    validate_opaque_id(attachment_id)?;
    sqlx::query_as::<_, LocalAttachment>(
        "SELECT
           attachment.id AS attachment_id,
           attachment.session_id,
           attachment.workspace_id,
           attachment.relative_path,
           attachment.source_type,
           attachment.sha256,
           attachment.size_bytes
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
            session_id: "session-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            expected_sha256: "a".repeat(64),
            expected_size_bytes: 1,
            object_key: "owner/object.anb1".to_string(),
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

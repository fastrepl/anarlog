#[cfg(test)]
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
#[cfg(test)]
use std::time::{Duration, SystemTime};

use anlg_e2ee::{
    AttachmentBlobCiphertextMetadata, AttachmentBlobContext, AttachmentBlobMetadata,
    AttachmentBlobPlaintextMetadata, WorkspaceKey,
};
#[cfg(test)]
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use tauri::Runtime;
use tauri_plugin_settings::SettingsPluginExt;
use uuid::{Uuid, Version};

use crate::control::DownloadOperation;
use crate::error::{Error, Result};
use crate::models::{
    PreparedSharedUpload, PreparedUpload, RestoredAttachment, SharedAttachmentCacheResult,
    UploadDescriptor,
};

mod cache;
mod delete_guard;
mod download;

use cache::{
    CacheFileGuard, cached_shared_attachment_path, cleanup_shared_upload_path,
    clear_attachment_cache_directory, commit_shared_cache_entry, create_shared_upload_cache_root,
    ensure_file_size, file_matches_async, file_matches_cancellable_async, private_cache_path,
    private_cache_root, read_range, read_shared_cache_metadata, shared_cache_id,
    shared_cache_matches_async, shared_cache_metadata_path, shared_cache_root,
    shared_preview_cache_root, shared_scope_path, shared_upload_cache_path,
    shared_upload_cache_root, snapshot_verified_file, valid_cache_id, valid_sha256, validate_range,
};
#[cfg(test)]
use cache::{
    MAX_RANGE_BYTES, file_matches, hash_identifier, hex_digest, shared_cache_matches,
    write_shared_cache_metadata,
};
#[cfg(test)]
use delete_guard::{
    DELETE_GUARD_ORPHAN_GRACE, DeleteGuardReconcile, DeleteSourcePreflight,
    create_delete_guard_root, delete_source_attachment, exact_delete_dependency,
    reconcile_delete_guard_files, reconcile_staged_delete_guard,
    reconcile_staged_delete_guard_once, seal_delete_guard, stage_delete_guard_restore,
};
pub use delete_guard::{commit_delete_guard, prepare_delete_guard, reconcile_delete_guards};
use download::{
    DownloadObject, download_to_path, persist_staged_attachment, require_configured_supabase_url,
    stage_attachment_restore, validate_signed_download_url,
};

const FORMAT_VERSION: i16 = 1;
const MAX_PLAINTEXT_BYTES: u64 = anlg_e2ee::ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES;
const MAX_CIPHERTEXT_BYTES: u64 = 545_259_520;
pub(crate) const SHARED_PREVIEW_SCOPE_PREFIX: &str = "preview:";

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

    let (attachment_ref, version_ref) = attachment_backup_refs(
        &key,
        &record.workspace_id,
        &record.attachment_id,
        &plaintext,
    )?;

    Ok(UploadDescriptor {
        attachment_ref,
        version_ref,
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

pub(crate) fn clear_shared_attachment_preview_cache_root<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<()> {
    clear_attachment_cache_directory(&shared_preview_cache_root(app)?)
}

pub async fn clear_shared_attachment_preview_scopes<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<bool> {
    clear_shared_attachment_preview_scopes_at(&shared_preview_cache_root(app)?).await
}

async fn clear_shared_attachment_preview_scopes_at(path: &Path) -> Result<bool> {
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        tokio::fs::remove_dir_all(path).await?;
    } else {
        tokio::fs::remove_file(path).await?;
    }
    Ok(true)
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

fn attachment_backup_refs(
    key: &WorkspaceKey,
    workspace_id: &str,
    attachment_id: &str,
    plaintext: &AttachmentBlobPlaintextMetadata,
) -> Result<(String, String)> {
    Ok((
        key.blind_attachment_backup_ref(workspace_id, attachment_id)?,
        key.blind_attachment_backup_version_ref(workspace_id, attachment_id, plaintext)?,
    ))
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
    let session_candidate = anlg_fs_sync_core::FsSyncCore::new(vault_base.clone())
        .resolve_session_dir(&attachment.session_id)
        .map_err(|_| Error::LocalAttachmentUnavailable)?;
    let session_dir = anlg_fs_sync_core::resolve_path_inside_base(&vault_base, &session_candidate)
        .map_err(|_| Error::LocalAttachmentUnavailable)?;
    let path = anlg_fs_sync_core::resolve_path_inside_base(
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

#[cfg(test)]
mod tests;

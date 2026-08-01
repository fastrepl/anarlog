use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anlg_e2ee::{
    AttachmentBlobCiphertextMetadata, AttachmentBlobContext, AttachmentBlobMetadata,
    AttachmentBlobPlaintextMetadata, WorkspaceKey,
};
use sqlx::FromRow;
use tauri::{Manager, Runtime};
use uuid::Uuid;

use super::cache::{
    cleanup_shared_upload_path, file_matches, file_matches_cancellable,
    file_matches_cancellable_async, sync_destination_directory, valid_cache_id, valid_sha256,
};
use super::{
    FORMAT_VERSION, LocalAttachment, MAX_CIPHERTEXT_BYTES, attachment_backup_refs,
    attachment_paths, plaintext_metadata, private_object_id, resolve_attachment_path,
    valid_plaintext_size, validate_opaque_id, workspace_key,
};
use crate::control::DownloadOperation;
use crate::error::{Error, Result};
use crate::models::PreparedDeleteGuard;

pub(super) const DELETE_GUARD_ORPHAN_GRACE: Duration = Duration::from_secs(15 * 60);
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
   attachment.deleted_at,
   COALESCE(local.availability, 'absent') AS local_availability
 FROM attachment_transfer_jobs AS job
 LEFT JOIN session_attachments AS attachment
   ON attachment.id = job.attachment_id
  AND attachment.session_id = job.session_id
  AND attachment.workspace_id = job.workspace_id
 LEFT JOIN attachment_local_state AS local
   ON local.attachment_id = attachment.id
  AND local.session_id = attachment.session_id
  AND local.relative_path = attachment.relative_path
 WHERE job.id = ? AND job.attempt_count = ?
   AND job.direction = 'delete' AND job.phase = 'finalizing'
 LIMIT 1";
#[derive(Debug, Clone, FromRow)]
pub(super) struct DeleteSourcePreflight {
    pub(super) attachment_id: String,
    pub(super) session_id: String,
    pub(super) workspace_id: String,
    pub(super) expected_sha256: String,
    pub(super) expected_size_bytes: i64,
    pub(super) object_key: String,
    pub(super) cache_id: String,
    pub(super) ciphertext_sha256: String,
    pub(super) ciphertext_size_bytes: i64,
    pub(super) current_attachment_id: Option<String>,
    pub(super) relative_path: Option<String>,
    pub(super) source_type: Option<String>,
    pub(super) attachment_sha256: Option<String>,
    pub(super) attachment_size_bytes: Option<i64>,
    pub(super) attachment_cloud_object_key: Option<String>,
    pub(super) cloud_sync_enabled: Option<i64>,
    pub(super) deleted_at: Option<String>,
    pub(super) local_availability: String,
}

pub(super) struct DeleteGuardFileGuard {
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

    pub(super) fn disarm(mut self) {
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

pub async fn prepare_delete_guard<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri_plugin_db::ManagedState,
    operation: &DownloadOperation,
    job_id: &str,
    attempt_count: i64,
    create_guard: bool,
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
    if !valid_sha256(&record.expected_sha256) || record.object_key.is_empty() {
        return Err(Error::InvalidMetadata);
    }
    let expected = plaintext_metadata(&record.expected_sha256, record.expected_size_bytes)?;
    let expected_size = expected.size_bytes;
    let key = workspace_key(state, &record.workspace_id)?;
    let (attachment_ref, version_ref) =
        attachment_backup_refs(&key, &record.workspace_id, &record.attachment_id, &expected)?;
    let prepared = |should_delete, guard_id| PreparedDeleteGuard {
        should_delete,
        guard_id,
        attachment_ref: attachment_ref.clone(),
        version_ref: version_ref.clone(),
    };
    if !create_guard {
        return Ok(prepared(false, String::new()));
    }
    let Some(attachment) = delete_source_attachment(&record)? else {
        clear_delete_guard_link(state.pool(), operation, job_id, attempt_count, &record).await?;
        cleanup_linked_delete_guard(app, &record.cache_id).await;
        return Ok(prepared(true, String::new()));
    };

    let object_id = private_object_id(&record.object_key)?;
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
            return Ok(prepared(true, record.cache_id));
        }
    }

    let source_path = match resolve_attachment_path(app, &attachment, true) {
        Ok(path) => path,
        Err(Error::LocalAttachmentUnavailable) => {
            clear_delete_guard_link(state.pool(), operation, job_id, attempt_count, &record)
                .await?;
            cleanup_linked_delete_guard(app, &record.cache_id).await;
            return Ok(prepared(false, String::new()));
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
        return Ok(prepared(false, String::new()));
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
            return Ok(prepared(false, String::new()));
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
    Ok(prepared(true, guard_id))
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
        if exact_delete_dependency(&current)? {
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
pub(super) fn delete_source_attachment(
    record: &DeleteSourcePreflight,
) -> Result<Option<LocalAttachment>> {
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

pub(super) fn exact_delete_dependency(record: &DeleteSourcePreflight) -> Result<bool> {
    if delete_source_attachment(record)?.is_none() {
        return Ok(false);
    }
    Ok(record.cloud_sync_enabled == Some(1) || record.local_availability != "present")
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
        | Error::E2ee(anlg_e2ee::AttachmentBlobError::SourceMismatch) => true,
        Error::Io(source) => source.kind() == std::io::ErrorKind::NotFound,
        _ => false,
    }
}

pub(super) fn seal_delete_guard(
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
        Err(anlg_e2ee::AttachmentBlobError::Io(error))
            if error.kind() == std::io::ErrorKind::ConnectionAborted =>
        {
            return Err(Error::Cancelled);
        }
        Err(error) => return Err(error.into()),
    };
    destination.sync_all()?;
    Ok((metadata, guard))
}

pub(super) enum DeleteGuardReconcile {
    Ready(Vec<PathBuf>),
    Retry {
        staged: tempfile::NamedTempFile,
        conflicts: Vec<PathBuf>,
        error: Error,
    },
}

#[cfg(test)]
pub(super) fn reconcile_staged_delete_guard(
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

pub(super) fn reconcile_staged_delete_guard_once(
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

fn delete_guard_root<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|_| Error::CacheUnavailable)?
        .join("attachment-sync")
        .join("delete-guards"))
}

pub(super) async fn create_delete_guard_root(path: &Path) -> Result<()> {
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

pub(super) fn reconcile_delete_guard_files(
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

pub(super) fn stage_delete_guard_restore(
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
        Err(anlg_e2ee::AttachmentBlobError::Io(error))
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

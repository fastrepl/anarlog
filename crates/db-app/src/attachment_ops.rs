use sqlx::SqliteConnection;

const REPLACED_ATTACHMENT_DELETE_BY_PATH_SQL: &str =
    "INSERT OR IGNORE INTO attachment_transfer_jobs (
  id, attachment_id, session_id, workspace_id, direction,
  expected_sha256, expected_size_bytes, object_key
)
SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
  'delete', attachment.sha256, attachment.size_bytes, attachment.cloud_object_key
FROM session_attachments AS attachment
WHERE attachment.session_id = ?
  AND attachment.relative_path = ?
  AND (attachment.sha256 <> ? OR attachment.size_bytes <> ?)
  AND attachment.cloud_object_key <> ''
ORDER BY attachment.deleted_at IS NULL DESC, attachment.updated_at DESC, attachment.id
LIMIT 1";

const REPLACED_ATTACHMENT_DELETE_BY_ID_SQL: &str =
    "INSERT OR IGNORE INTO attachment_transfer_jobs (
  id, attachment_id, session_id, workspace_id, direction,
  expected_sha256, expected_size_bytes, object_key
)
SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
  'delete', attachment.sha256, attachment.size_bytes, attachment.cloud_object_key
FROM session_attachments AS attachment
WHERE attachment.session_id = ?
  AND attachment.id = ?
  AND (attachment.sha256 <> ? OR attachment.size_bytes <> ?)
  AND attachment.cloud_object_key <> ''
ORDER BY attachment.deleted_at IS NULL DESC, attachment.updated_at DESC, attachment.id
LIMIT 1";

const ATTACHMENT_UPLOAD_BY_PATH_SQL: &str = "INSERT OR IGNORE INTO attachment_transfer_jobs (
  id, attachment_id, session_id, workspace_id, direction,
  expected_sha256, expected_size_bytes
)
SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
  'upload', attachment.sha256, attachment.size_bytes
FROM session_attachments AS attachment
JOIN attachment_local_state AS local
  ON local.attachment_id = attachment.id AND local.availability = 'present'
WHERE attachment.session_id = ?
  AND attachment.relative_path = ?
  AND attachment.cloud_sync_enabled = 1
  AND attachment.cloud_object_key = ''
  AND attachment.deleted_at IS NULL
ORDER BY attachment.updated_at DESC, attachment.id
LIMIT 1";

const ATTACHMENT_UPLOAD_BY_ID_SQL: &str = "INSERT OR IGNORE INTO attachment_transfer_jobs (
  id, attachment_id, session_id, workspace_id, direction,
  expected_sha256, expected_size_bytes
)
SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
  'upload', attachment.sha256, attachment.size_bytes
FROM session_attachments AS attachment
JOIN attachment_local_state AS local
  ON local.attachment_id = attachment.id AND local.availability = 'present'
WHERE attachment.session_id = ?
  AND attachment.id = ?
  AND attachment.cloud_sync_enabled = 1
  AND attachment.cloud_object_key = ''
  AND attachment.deleted_at IS NULL
ORDER BY attachment.updated_at DESC, attachment.id
LIMIT 1";

pub async fn enqueue_replaced_attachment_delete_by_path(
    conn: &mut SqliteConnection,
    job_id: &str,
    session_id: &str,
    relative_path: &str,
    next_sha256: &str,
    next_size_bytes: i64,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(REPLACED_ATTACHMENT_DELETE_BY_PATH_SQL)
        .bind(job_id)
        .bind(session_id)
        .bind(relative_path)
        .bind(next_sha256)
        .bind(next_size_bytes)
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}

pub async fn enqueue_replaced_attachment_delete_by_id(
    conn: &mut SqliteConnection,
    job_id: &str,
    session_id: &str,
    attachment_id: &str,
    next_sha256: &str,
    next_size_bytes: i64,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(REPLACED_ATTACHMENT_DELETE_BY_ID_SQL)
        .bind(job_id)
        .bind(session_id)
        .bind(attachment_id)
        .bind(next_sha256)
        .bind(next_size_bytes)
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}

#[allow(clippy::too_many_arguments)]
pub async fn update_note_attachment(
    conn: &mut SqliteConnection,
    filename: &str,
    content_type: &str,
    size_bytes: i64,
    sha256: &str,
    source_id: &str,
    session_id: &str,
    relative_path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_attachments
        SET
          filename = ?,
          content_type = ?,
          size_bytes = ?,
          cloud_object_key = CASE
            WHEN session_attachments.sha256 = ?
              AND session_attachments.size_bytes = ? THEN cloud_object_key
            ELSE ''
          END,
          storage_kind = CASE
            WHEN session_attachments.sha256 = ?
              AND session_attachments.size_bytes = ? THEN storage_kind
            ELSE 'local_file'
          END,
          sha256 = ?,
          source_type = 'note_upload',
          source_id = ?,
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
          deleted_at = NULL
        WHERE id = (
          SELECT attachment.id
          FROM session_attachments AS attachment
          JOIN sessions AS session
            ON session.id = attachment.session_id
            AND session.deleted_at IS NULL
          WHERE attachment.session_id = ?
            AND attachment.relative_path = ?
          ORDER BY attachment.deleted_at IS NULL DESC,
            attachment.updated_at DESC,
            attachment.id
          LIMIT 1
        )",
    )
    .bind(filename)
    .bind(content_type)
    .bind(size_bytes)
    .bind(sha256)
    .bind(size_bytes)
    .bind(sha256)
    .bind(size_bytes)
    .bind(sha256)
    .bind(source_id)
    .bind(session_id)
    .bind(relative_path)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_note_attachment(
    conn: &mut SqliteConnection,
    id: &str,
    filename: &str,
    relative_path: &str,
    content_type: &str,
    size_bytes: i64,
    sha256: &str,
    source_id: &str,
    session_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO session_attachments (
          id,
          workspace_id,
          session_id,
          filename,
          relative_path,
          content_type,
          size_bytes,
          sha256,
          storage_kind,
          cloud_object_key,
          source_type,
          source_id,
          metadata_json
        )
        SELECT
          ?,
          session.workspace_id,
          session.id,
          ?,
          ?,
          ?,
          ?,
          ?,
          'local_file',
          '',
          'note_upload',
          ?,
          '{}'
        FROM sessions AS session
        WHERE session.id = ?
          AND session.deleted_at IS NULL
          AND NOT EXISTS (
            SELECT 1
            FROM session_attachments AS attachment
            WHERE attachment.session_id = session.id
              AND attachment.relative_path = ?
              AND attachment.deleted_at IS NULL
          )",
    )
    .bind(id)
    .bind(filename)
    .bind(relative_path)
    .bind(content_type)
    .bind(size_bytes)
    .bind(sha256)
    .bind(source_id)
    .bind(session_id)
    .bind(relative_path)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn upsert_attachment_local_state_by_path(
    conn: &mut SqliteConnection,
    session_id: &str,
    relative_path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO attachment_local_state (
          attachment_id,
          session_id,
          relative_path,
          availability,
          updated_at
        )
        SELECT
          attachment.id,
          attachment.session_id,
          attachment.relative_path,
          'present',
          strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        FROM session_attachments AS attachment
        WHERE attachment.session_id = ?
          AND attachment.relative_path = ?
          AND attachment.deleted_at IS NULL
        ORDER BY attachment.updated_at DESC, attachment.id
        LIMIT 1
        ON CONFLICT(attachment_id) DO UPDATE SET
          session_id = excluded.session_id,
          relative_path = excluded.relative_path,
          availability = excluded.availability,
          updated_at = excluded.updated_at",
    )
    .bind(session_id)
    .bind(relative_path)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn upsert_attachment_local_state_by_id(
    conn: &mut SqliteConnection,
    attachment_id: &str,
    session_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO attachment_local_state (
          attachment_id,
          session_id,
          relative_path,
          availability,
          updated_at
        )
        SELECT
          attachment.id,
          attachment.session_id,
          attachment.relative_path,
          'present',
          strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        FROM session_attachments AS attachment
        WHERE attachment.id = ?
          AND attachment.session_id = ?
          AND attachment.deleted_at IS NULL
        ON CONFLICT(attachment_id) DO UPDATE SET
          session_id = excluded.session_id,
          relative_path = excluded.relative_path,
          availability = excluded.availability,
          updated_at = excluded.updated_at",
    )
    .bind(attachment_id)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn enqueue_attachment_upload_by_path(
    conn: &mut SqliteConnection,
    job_id: &str,
    session_id: &str,
    relative_path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(ATTACHMENT_UPLOAD_BY_PATH_SQL)
        .bind(job_id)
        .bind(session_id)
        .bind(relative_path)
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}

pub async fn enqueue_attachment_upload_by_id(
    conn: &mut SqliteConnection,
    job_id: &str,
    session_id: &str,
    attachment_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(ATTACHMENT_UPLOAD_BY_ID_SQL)
        .bind(job_id)
        .bind(session_id)
        .bind(attachment_id)
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}

#[allow(clippy::too_many_arguments)]
pub async fn update_session_audio_attachment(
    conn: &mut SqliteConnection,
    filename: &str,
    content_type: &str,
    size_bytes: i64,
    sha256: &str,
    attachment_id: &str,
    session_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_attachments
        SET
          filename = ?,
          relative_path = ?,
          content_type = ?,
          size_bytes = ?,
          cloud_object_key = CASE
            WHEN session_attachments.sha256 = ?
              AND session_attachments.size_bytes = ? THEN cloud_object_key
            ELSE ''
          END,
          storage_kind = CASE
            WHEN session_attachments.sha256 = ?
              AND session_attachments.size_bytes = ? THEN storage_kind
            ELSE 'local_file'
          END,
          sha256 = ?,
          source_type = 'session_audio',
          source_id = 'primary',
          metadata_json = json_set(
            CASE
              WHEN json_valid(metadata_json) THEN metadata_json
              ELSE '{}'
            END,
            '$.transcript_status',
            'processing'
          ),
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
          deleted_at = NULL
        WHERE id = ?
          AND session_id = ?
          AND EXISTS (
            SELECT 1
            FROM sessions AS session
            WHERE session.id = ?
              AND session.deleted_at IS NULL
          )",
    )
    .bind(filename)
    .bind(filename)
    .bind(content_type)
    .bind(size_bytes)
    .bind(sha256)
    .bind(size_bytes)
    .bind(sha256)
    .bind(size_bytes)
    .bind(sha256)
    .bind(attachment_id)
    .bind(session_id)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_session_audio_attachment(
    conn: &mut SqliteConnection,
    attachment_id: &str,
    filename: &str,
    content_type: &str,
    size_bytes: i64,
    sha256: &str,
    session_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO session_attachments (
          id,
          workspace_id,
          session_id,
          filename,
          relative_path,
          content_type,
          size_bytes,
          sha256,
          storage_kind,
          cloud_object_key,
          source_type,
          source_id,
          metadata_json
        )
        SELECT
          ?,
          session.workspace_id,
          session.id,
          ?,
          ?,
          ?,
          ?,
          ?,
          'local_file',
          '',
          'session_audio',
          'primary',
          json_object('transcript_status', 'processing')
        FROM sessions AS session
        WHERE session.id = ?
          AND session.deleted_at IS NULL
          AND NOT EXISTS (
            SELECT 1
            FROM session_attachments AS attachment
            WHERE attachment.id = ?
          )",
    )
    .bind(attachment_id)
    .bind(filename)
    .bind(filename)
    .bind(content_type)
    .bind(size_bytes)
    .bind(sha256)
    .bind(session_id)
    .bind(attachment_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn set_attachment_cloud_sync_flag(
    conn: &mut SqliteConnection,
    enabled: i64,
    now: &str,
    attachment_id: &str,
    session_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_attachments
        SET cloud_sync_enabled = ?, updated_at = ?
        WHERE id = ? AND session_id = ? AND deleted_at IS NULL",
    )
    .bind(enabled)
    .bind(now)
    .bind(attachment_id)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn complete_attachment_delete_jobs(
    conn: &mut SqliteConnection,
    now: &str,
    attachment_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE attachment_transfer_jobs
        SET phase = 'completed', completed_at = ?, updated_at = ?, last_error = ''
        WHERE attachment_id = ?
          AND direction IN ('delete')
          AND phase IN ('queued', 'retry_wait', 'failed')",
    )
    .bind(now)
    .bind(now)
    .bind(attachment_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn complete_attachment_upload_download_jobs(
    conn: &mut SqliteConnection,
    now: &str,
    attachment_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE attachment_transfer_jobs
        SET phase = 'completed', completed_at = ?, updated_at = ?, last_error = ''
        WHERE attachment_id = ?
          AND direction IN ('upload', 'download')
          AND phase IN ('queued', 'retry_wait', 'failed')",
    )
    .bind(now)
    .bind(now)
    .bind(attachment_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn enqueue_attachment_download_job_enabled(
    conn: &mut SqliteConnection,
    job_id: &str,
    attachment_id: &str,
    session_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO attachment_transfer_jobs (
          id,
          attachment_id,
          session_id,
          workspace_id,
          direction,
          expected_sha256,
          expected_size_bytes,
          object_key
        )
        SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
          'download', attachment.sha256, attachment.size_bytes,
          attachment.cloud_object_key
        FROM session_attachments AS attachment
        LEFT JOIN attachment_local_state AS local
          ON local.attachment_id = attachment.id
        WHERE attachment.id = ?
          AND attachment.session_id = ?
          AND attachment.cloud_sync_enabled = 1
          AND attachment.cloud_object_key <> ''
          AND attachment.deleted_at IS NULL
          AND COALESCE(local.availability, 'absent') <> 'present'",
    )
    .bind(job_id)
    .bind(attachment_id)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn enqueue_attachment_download_job_disabled(
    conn: &mut SqliteConnection,
    job_id: &str,
    attachment_id: &str,
    session_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO attachment_transfer_jobs (
          id,
          attachment_id,
          session_id,
          workspace_id,
          direction,
          expected_sha256,
          expected_size_bytes,
          object_key
        )
        SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
          'download', attachment.sha256, attachment.size_bytes,
          attachment.cloud_object_key
        FROM session_attachments AS attachment
        LEFT JOIN attachment_local_state AS local
          ON local.attachment_id = attachment.id
        WHERE attachment.id = ?
          AND attachment.session_id = ?
          AND attachment.cloud_sync_enabled = 0
          AND attachment.cloud_object_key <> ''
          AND attachment.deleted_at IS NULL
          AND COALESCE(local.availability, 'absent') <> 'present'",
    )
    .bind(job_id)
    .bind(attachment_id)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn enqueue_attachment_delete_job_disabled(
    conn: &mut SqliteConnection,
    job_id: &str,
    attachment_id: &str,
    session_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO attachment_transfer_jobs (
          id,
          attachment_id,
          session_id,
          workspace_id,
          direction,
          expected_sha256,
          expected_size_bytes,
          object_key
        )
        SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
          'delete', attachment.sha256, attachment.size_bytes,
          attachment.cloud_object_key
        FROM session_attachments AS attachment
        JOIN attachment_local_state AS local
          ON local.attachment_id = attachment.id
          AND local.availability = 'present'
        WHERE attachment.id = ?
          AND attachment.session_id = ?
          AND attachment.cloud_sync_enabled = 0
          AND attachment.cloud_object_key <> ''
          AND attachment.deleted_at IS NULL",
    )
    .bind(job_id)
    .bind(attachment_id)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn tombstone_session_audio_attachment(
    conn: &mut SqliteConnection,
    attachment_id: &str,
    session_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_attachments
        SET
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
          deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?
          AND session_id = ?
          AND deleted_at IS NULL",
    )
    .bind(attachment_id)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

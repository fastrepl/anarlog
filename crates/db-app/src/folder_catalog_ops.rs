use sqlx::SqliteConnection;

#[allow(clippy::too_many_arguments)]
pub async fn update_folder_attachment(
    conn: &mut SqliteConnection,
    filename: &str,
    content_type: &str,
    size_bytes: i64,
    sha256: &str,
    source_id: &str,
    folder_path: &str,
    relative_path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE folder_attachments
        SET
          filename = ?,
          content_type = ?,
          size_bytes = ?,
          sha256 = ?,
          source_type = 'folder_material',
          source_id = ?,
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
          deleted_at = NULL
        WHERE id = (
          SELECT id
          FROM folder_attachments
          WHERE folder_path = ?
            AND relative_path = ?
          ORDER BY deleted_at IS NULL DESC,
            updated_at DESC,
            id
          LIMIT 1
        )",
    )
    .bind(filename)
    .bind(content_type)
    .bind(size_bytes)
    .bind(sha256)
    .bind(source_id)
    .bind(folder_path)
    .bind(relative_path)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_folder_attachment(
    conn: &mut SqliteConnection,
    id: &str,
    folder_path: &str,
    filename: &str,
    relative_path: &str,
    content_type: &str,
    size_bytes: i64,
    sha256: &str,
    source_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO folder_attachments (
          id,
          workspace_id,
          folder_path,
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
          COALESCE((
            SELECT session.workspace_id
            FROM sessions AS session
            WHERE session.deleted_at IS NULL
              AND (session.folder_path = ? OR session.folder_path LIKE ?)
            ORDER BY session.updated_at DESC, session.id
            LIMIT 1
          ), ''),
          ?,
          ?,
          ?,
          ?,
          ?,
          ?,
          'local_file',
          '',
          'folder_material',
          ?,
          '{}'
        WHERE NOT EXISTS (
          SELECT 1
          FROM folder_attachments
          WHERE folder_path = ?
            AND relative_path = ?
            AND deleted_at IS NULL
        )",
    )
    .bind(id)
    .bind(folder_path)
    .bind(format!("{folder_path}/%"))
    .bind(folder_path)
    .bind(filename)
    .bind(relative_path)
    .bind(content_type)
    .bind(size_bytes)
    .bind(sha256)
    .bind(source_id)
    .bind(folder_path)
    .bind(relative_path)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn tombstone_folder_attachment(
    conn: &mut SqliteConnection,
    folder_path: &str,
    relative_path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE folder_attachments
        SET
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
          deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE folder_path = ?
          AND relative_path = ?
          AND deleted_at IS NULL",
    )
    .bind(folder_path)
    .bind(relative_path)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn revive_folder(conn: &mut SqliteConnection, path: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE folders
        SET
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
          deleted_at = NULL
        WHERE id = (
          SELECT id
          FROM folders
          WHERE path = ?
          ORDER BY deleted_at IS NULL DESC,
            updated_at DESC,
            id
          LIMIT 1
        )",
    )
    .bind(path)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn insert_folder_if_missing(
    conn: &mut SqliteConnection,
    id: &str,
    path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO folders (
          id,
          workspace_id,
          path
        )
        SELECT
          ?,
          COALESCE((
            SELECT session.workspace_id
            FROM sessions AS session
            WHERE session.deleted_at IS NULL
              AND (session.folder_path = ? OR session.folder_path LIKE ?)
            ORDER BY session.updated_at DESC, session.id
            LIMIT 1
          ), ''),
          ?
        WHERE NOT EXISTS (
          SELECT 1
          FROM folders
          WHERE path = ?
            AND deleted_at IS NULL
        )",
    )
    .bind(id)
    .bind(path)
    .bind(format!("{path}/%"))
    .bind(path)
    .bind(path)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn rename_folder_row(
    conn: &mut SqliteConnection,
    new_path: &str,
    old_path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE folders
        SET
          path = ?,
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
          deleted_at = NULL
        WHERE path = ?
          AND deleted_at IS NULL",
    )
    .bind(new_path)
    .bind(old_path)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn rewrite_folder_attachment_paths(
    conn: &mut SqliteConnection,
    old_path: &str,
    new_path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE folder_attachments
        SET
          folder_path = CASE
            WHEN folder_path = ? THEN ?
            ELSE ? || substr(folder_path, length(?) + 1)
          END,
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE deleted_at IS NULL
          AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)",
    )
    .bind(old_path)
    .bind(new_path)
    .bind(new_path)
    .bind(old_path)
    .bind(old_path)
    .bind(format!("{old_path}/%"))
    .bind(format!("{old_path}\\%"))
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn rewrite_session_folder_paths(
    conn: &mut SqliteConnection,
    old_path: &str,
    new_path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE sessions
        SET
          folder_path = CASE
            WHEN folder_path = ? THEN ?
            ELSE ? || substr(folder_path, length(?) + 1)
          END,
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE deleted_at IS NULL
          AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)",
    )
    .bind(old_path)
    .bind(new_path)
    .bind(new_path)
    .bind(old_path)
    .bind(old_path)
    .bind(format!("{old_path}/%"))
    .bind(format!("{old_path}\\%"))
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn tombstone_folders_under(
    conn: &mut SqliteConnection,
    path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE folders
        SET
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
          deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE deleted_at IS NULL
          AND (path = ? OR path LIKE ? OR path LIKE ?)",
    )
    .bind(path)
    .bind(format!("{path}/%"))
    .bind(format!("{path}\\%"))
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn tombstone_folder_attachments_under(
    conn: &mut SqliteConnection,
    path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE folder_attachments
        SET
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
          deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE deleted_at IS NULL
          AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)",
    )
    .bind(path)
    .bind(format!("{path}/%"))
    .bind(format!("{path}\\%"))
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn clear_session_folder_paths(
    conn: &mut SqliteConnection,
    path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE sessions
        SET
          folder_path = '',
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE deleted_at IS NULL
          AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)",
    )
    .bind(path)
    .bind(format!("{path}/%"))
    .bind(format!("{path}\\%"))
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn update_folder_instructions(
    conn: &mut SqliteConnection,
    path: &str,
    instructions: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE folders
        SET
          instructions = ?,
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE path = ?
          AND deleted_at IS NULL",
    )
    .bind(instructions)
    .bind(path)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn update_folder_workspace(
    conn: &mut SqliteConnection,
    path: &str,
    workspace_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE folders
        SET
          workspace_id = ?,
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE path = ?
          AND deleted_at IS NULL",
    )
    .bind(workspace_id)
    .bind(path)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn update_folder_icon(
    conn: &mut SqliteConnection,
    path: &str,
    icon_json: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE folders
        SET
          icon_json = ?,
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE path = ?
          AND deleted_at IS NULL",
    )
    .bind(icon_json)
    .bind(path)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

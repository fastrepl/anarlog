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
              AND (session.folder_path = ? OR substr(session.folder_path, 1, length(?) + 1) = ? || '/')
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
    .bind(folder_path)
    .bind(folder_path)
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
              AND (session.folder_path = ? OR substr(session.folder_path, 1, length(?) + 1) = ? || '/')
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
    .bind(path)
    .bind(path)
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
          AND (folder_path = ? OR substr(folder_path, 1, length(?) + 1) IN (? || '/', ? || '\\'))",
    )
    .bind(old_path)
    .bind(new_path)
    .bind(new_path)
    .bind(old_path)
    .bind(old_path)
    .bind(old_path)
    .bind(old_path)
    .bind(old_path)
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
          AND (folder_path = ? OR substr(folder_path, 1, length(?) + 1) IN (? || '/', ? || '\\'))",
    )
    .bind(old_path)
    .bind(new_path)
    .bind(new_path)
    .bind(old_path)
    .bind(old_path)
    .bind(old_path)
    .bind(old_path)
    .bind(old_path)
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
          AND (path = ? OR substr(path, 1, length(?) + 1) IN (? || '/', ? || '\\'))",
    )
    .bind(path)
    .bind(path)
    .bind(path)
    .bind(path)
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
          AND (folder_path = ? OR substr(folder_path, 1, length(?) + 1) IN (? || '/', ? || '\\'))",
    )
    .bind(path)
    .bind(path)
    .bind(path)
    .bind(path)
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
          AND (folder_path = ? OR substr(folder_path, 1, length(?) + 1) IN (? || '/', ? || '\\'))",
    )
    .bind(path)
    .bind(path)
    .bind(path)
    .bind(path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_db_core::Db;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        crate::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, folder_path) VALUES
               ('own', 'Q1_2026'), ('child', 'Q1_2026/Notes'), ('lookalike', 'Q1-2026/Notes'),
               ('cased', 'q1_2026/Notes'), ('percent', 'Q1%2026/Notes')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO folders (id, path) VALUES ('child', 'Q1_2026/Notes'), ('lookalike', 'Q1-2026/Notes')")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO folder_attachments (id, folder_path) VALUES
               ('child', 'Q1_2026/Notes'), ('lookalike', 'Q1-2026/Notes'), ('cased', 'q1_2026/Notes')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db
    }

    async fn live_attachments(db: &Db) -> Vec<(String, String)> {
        sqlx::query_as(
            "SELECT id, folder_path FROM folder_attachments WHERE deleted_at IS NULL ORDER BY id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap()
    }

    async fn session_folders(db: &Db) -> Vec<(String, String)> {
        sqlx::query_as("SELECT id, folder_path FROM sessions ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap()
    }

    fn pairs(rows: &[(&str, &str)]) -> Vec<(String, String)> {
        rows.iter()
            .map(|(id, path)| (id.to_string(), path.to_string()))
            .collect()
    }

    // `_` and `%` are LIKE wildcards and LIKE ignores ASCII case, so renaming or deleting
    // `Q1_2026` used to reach `Q1-2026`, `Q1%2026` and `q1_2026` too.
    #[tokio::test]
    async fn folder_operations_match_paths_literally() {
        let db = test_db().await;
        let mut conn = db.pool().acquire().await.unwrap();
        rewrite_session_folder_paths(&mut conn, "Q1_2026", "Archive")
            .await
            .unwrap();
        rewrite_folder_attachment_paths(&mut conn, "Q1_2026", "Archive")
            .await
            .unwrap();
        drop(conn);
        assert_eq!(
            session_folders(&db).await,
            pairs(&[
                ("cased", "q1_2026/Notes"),
                ("child", "Archive/Notes"),
                ("lookalike", "Q1-2026/Notes"),
                ("own", "Archive"),
                ("percent", "Q1%2026/Notes"),
            ])
        );
        assert_eq!(
            live_attachments(&db).await,
            pairs(&[
                ("cased", "q1_2026/Notes"),
                ("child", "Archive/Notes"),
                ("lookalike", "Q1-2026/Notes"),
            ])
        );

        let db = test_db().await;
        let mut conn = db.pool().acquire().await.unwrap();
        tombstone_folders_under(&mut conn, "Q1_2026").await.unwrap();
        tombstone_folder_attachments_under(&mut conn, "Q1_2026")
            .await
            .unwrap();
        clear_session_folder_paths(&mut conn, "Q1_2026")
            .await
            .unwrap();
        drop(conn);
        let live: Vec<String> =
            sqlx::query_scalar("SELECT id FROM folders WHERE deleted_at IS NULL ORDER BY id")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(live, ["lookalike"]);
        assert_eq!(
            live_attachments(&db).await,
            pairs(&[("cased", "q1_2026/Notes"), ("lookalike", "Q1-2026/Notes")])
        );
        assert_eq!(
            session_folders(&db).await,
            pairs(&[
                ("cased", "q1_2026/Notes"),
                ("child", ""),
                ("lookalike", "Q1-2026/Notes"),
                ("own", ""),
                ("percent", "Q1%2026/Notes"),
            ])
        );
    }
}

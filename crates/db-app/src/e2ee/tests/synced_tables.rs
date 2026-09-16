use super::*;

async fn binding_workspace(db: &anlg_db_core::Db) -> String {
    sqlx::query_scalar(
        "SELECT json_extract(value_json, '$.workspace_id')
         FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn tags_and_folders_written_without_a_workspace_take_the_device_workspace() {
    let db = test_db().await;
    let workspace = binding_workspace(&db).await;
    assert!(!workspace.is_empty());

    sqlx::query(
        "INSERT INTO tags (id, owner_user_id, name) VALUES ('planning', 'user-a', 'planning')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO folders (id, path) VALUES ('folder-1', 'Work')")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO daily_notes (id, owner_user_id, note_date, body)
         VALUES ('daily-1', 'user-a', '2026-09-09', 'today')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    for (table, id) in [
        ("tags", "planning"),
        ("folders", "folder-1"),
        ("daily_notes", "daily-1"),
    ] {
        let sql = format!("SELECT workspace_id FROM {table} WHERE id = ?");
        let stamped: String = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(id)
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(stamped, workspace, "{table} was not stamped");
        let dirty: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM e2ee_dirty_rows
             WHERE workspace_id = ? AND table_name = ? AND row_id = ?",
        )
        .bind(&workspace)
        .bind(table)
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(dirty, 1, "{table} was not queued for sync");
    }
}

#[tokio::test]
async fn previously_synced_tags_and_session_tags_keep_syncing_between_devices() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session-1', 'workspace-a', 'user-a', 'Tagged')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO tags (id, workspace_id, owner_user_id, name)
         VALUES ('planning', 'workspace-a', 'user-a', 'planning')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_tags (id, workspace_id, owner_user_id, session_id, tag_id)
         VALUES ('session-1:planning', 'workspace-a', 'user-a', 'session-1', 'planning')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO folders (id, workspace_id, path, instructions)
         VALUES ('folder-1', 'workspace-a', 'Work', 'Be brief')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    for (table, row_id) in [
        ("tags", "planning"),
        ("session_tags", "session-1:planning"),
        ("folders", "folder-1"),
    ] {
        seed_nightly_field(
            source.pool(),
            &workspace_keys,
            table,
            row_id,
            ROW_MANIFEST_FIELD,
            json!(true),
        )
        .await;
    }
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();

    let target = test_db().await;
    copy_replica(source.pool(), target.pool()).await;
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();

    let tag: (String, String) =
        sqlx::query_as("SELECT workspace_id, name FROM tags WHERE id = 'planning'")
            .fetch_one(target.pool())
            .await
            .unwrap();
    assert_eq!(tag, ("workspace-a".to_string(), "planning".to_string()));
    let session_tag: (String, String) = sqlx::query_as(
        "SELECT session_id, tag_id FROM session_tags WHERE id = 'session-1:planning'",
    )
    .fetch_one(target.pool())
    .await
    .unwrap();
    assert_eq!(
        session_tag,
        ("session-1".to_string(), "planning".to_string())
    );
    let folder: (String, String) =
        sqlx::query_as("SELECT path, instructions FROM folders WHERE id = 'folder-1'")
            .fetch_one(target.pool())
            .await
            .unwrap();
    assert_eq!(folder, ("Work".to_string(), "Be brief".to_string()));
}

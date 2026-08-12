use super::*;

#[tokio::test]
async fn encrypts_only_opaque_records() {
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Secret planning')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let stats = encrypt_e2ee_replica_changes(db.pool(), &keys("workspace-a"))
        .await
        .unwrap();
    let payloads: Vec<String> = sqlx::query_scalar("SELECT payload FROM e2ee_records")
        .fetch_all(db.pool())
        .await
        .unwrap();

    assert!(stats.encrypted_fields > 1);
    assert!(!payloads.is_empty());
    assert!(payloads.iter().all(|payload| {
        !payload.contains("Secret planning")
            && !payload.contains("session-1")
            && !payload.contains("sessions")
    }));
}

#[tokio::test]
async fn encrypts_and_reconstructs_attachment_metadata() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    sqlx::query(
        "INSERT INTO session_attachments (
               id, workspace_id, session_id, filename, relative_path,
               content_type, size_bytes, sha256, source_type, source_id
             ) VALUES (
               'attachment-1', 'workspace-a', 'session-1', 'secret-diagram.png',
               'attachments/secret-diagram.png', 'image/png', 42,
               'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
               'note_upload', 'secret-diagram.png'
             )",
    )
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO attachment_local_state (
               attachment_id, session_id, relative_path, availability
             ) VALUES (
               'attachment-1', 'session-1', 'local-only-secret.png', 'present'
             )",
    )
    .execute(source.pool())
    .await
    .unwrap();

    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    let payloads: Vec<String> =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE workspace_id = 'workspace-a'")
            .fetch_all(source.pool())
            .await
            .unwrap();
    assert!(!payloads.is_empty());
    assert!(payloads.iter().all(|payload| {
        !payload.contains("secret-diagram.png")
            && !payload.contains("attachments/secret-diagram.png")
            && !payload.contains("local-only-secret.png")
            && !payload.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    }));

    let target = test_db().await;
    copy_replica(source.pool(), target.pool()).await;
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    let attachment: (String, String, String, i64, String) = sqlx::query_as(
        "SELECT filename, relative_path, content_type, size_bytes, sha256
             FROM session_attachments WHERE id = 'attachment-1'",
    )
    .fetch_one(target.pool())
    .await
    .unwrap();
    assert_eq!(
        attachment,
        (
            "secret-diagram.png".to_string(),
            "attachments/secret-diagram.png".to_string(),
            "image/png".to_string(),
            42,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        )
    );
    let local_state_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM attachment_local_state")
        .fetch_one(target.pool())
        .await
        .unwrap();
    assert_eq!(local_state_count, 0);
}

#[tokio::test]
async fn reconstructs_every_protected_table_and_applies_deletions() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    for (table, id) in [
        ("action_items", "action-1"),
        ("humans", "human-1"),
        ("organizations", "organization-1"),
        ("session_attachments", "attachment-1"),
        ("session_documents", "document-1"),
        ("session_participants", "participant-1"),
        ("sessions", "session-1"),
        ("synced_preferences", "preference-1"),
        ("transcripts", "transcript-1"),
    ] {
        let sql = format!("INSERT INTO {table} (id, workspace_id) VALUES (?, 'workspace-a')");
        sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(id)
            .execute(source.pool())
            .await
            .unwrap();
    }
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();

    let target = test_db().await;
    copy_replica(source.pool(), target.pool()).await;
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    for table in E2EE_DOMAIN_TABLES {
        let sql = format!("SELECT COUNT(*) FROM {table} WHERE workspace_id = 'workspace-a'");
        let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_one(target.pool())
            .await
            .unwrap();
        assert_eq!(count, 1, "{table} was not reconstructed");
    }

    sqlx::query("DELETE FROM sessions WHERE id = 'session-1'")
        .execute(source.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    copy_replica(source.pool(), target.pool()).await;
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(target.pool())
        .await
        .unwrap();
    assert_eq!(sessions, 0);
}

#[tokio::test]
async fn applies_remote_changes_and_preserves_concurrent_local_edits() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'First')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();

    let target = test_db().await;
    let records: Vec<(String, String, String)> =
        sqlx::query_as("SELECT id, workspace_id, payload FROM e2ee_records")
            .fetch_all(source.pool())
            .await
            .unwrap();
    for (id, workspace_id, payload) in records {
        sqlx::query("INSERT INTO e2ee_records (id, workspace_id, payload) VALUES (?, ?, ?)")
            .bind(id)
            .bind(workspace_id)
            .bind(payload)
            .execute(target.pool())
            .await
            .unwrap();
    }
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(target.pool())
        .await
        .unwrap();
    assert_eq!(title, "First");

    sqlx::query("UPDATE sessions SET title = 'Remote' WHERE id = 'session-1'")
        .execute(source.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    let key = &workspace_keys["workspace-a"];
    let title_record_id = key.blind_field_id("sessions", "session-1", "title");
    let remote_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&title_record_id)
            .fetch_one(source.pool())
            .await
            .unwrap();
    sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(remote_payload)
        .bind(&title_record_id)
        .execute(target.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET title = 'Local' WHERE id = 'session-1'")
        .execute(target.pool())
        .await
        .unwrap();

    let stats = apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(target.pool())
        .await
        .unwrap();

    assert_eq!(title, "Local");
    assert_eq!(stats.skipped_local_changes, 1);
    assert!(stats.remaining_replica_changes);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM e2ee_replica_pending WHERE record_id = ?",
        )
        .bind(title_record_id)
        .fetch_one(target.pool())
        .await
        .unwrap(),
        1
    );
}

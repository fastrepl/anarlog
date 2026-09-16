use super::*;

#[tokio::test]
async fn new_transcripts_keep_whole_column_records_across_restore_and_edits() {
    let workspace_keys = keys("workspace-a");
    let key = workspace_keys["workspace-a"].active();
    let source = test_db().await;
    let target = test_db().await;
    let words = Value::Array(
        (0..600)
            .map(|index| json!({"text": format!("word-{index}")}))
            .collect(),
    );
    sqlx::query("INSERT INTO transcripts (id, workspace_id, words_json) VALUES ('transcript-1', 'workspace-a', ?)")
        .bind(words.to_string()).execute(source.pool()).await.unwrap();
    for (sender, receiver) in [(&source, &target), (&target, &source)] {
        encrypt_e2ee_replica_changes(sender.pool(), &workspace_keys)
            .await
            .unwrap();
        let fields: Vec<String> = sqlx::query_scalar(
            "SELECT field_name FROM e2ee_local_state WHERE table_name = 'transcripts'",
        )
        .fetch_all(sender.pool())
        .await
        .unwrap();
        assert!(fields.iter().any(|field| field == "words_json"));
        assert!(fields.iter().all(|field| !field.contains('#')));
        copy_replica(sender.pool(), receiver.pool()).await;
        apply_e2ee_replica_changes(receiver.pool(), &workspace_keys)
            .await
            .unwrap();
        let restored: String =
            sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'transcript-1'")
                .fetch_one(receiver.pool())
                .await
                .unwrap();
        assert_eq!(restored, words.to_string());
    }
    let edited_words = Value::Array(
        (0..650)
            .map(|index| json!({"text": format!("edited-{index}")}))
            .collect(),
    );
    sqlx::query("UPDATE transcripts SET words_json = ? WHERE id = 'transcript-1'")
        .bind(edited_words.to_string())
        .execute(target.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    let uploads = pending_e2ee_witness_uploads(target.pool(), "workspace-a", key, 100, 1024 * 1024)
        .await
        .unwrap();
    assert!(!uploads.is_empty());
    for upload in uploads {
        let field = key
            .open_field("workspace-a", &upload.record_id, &upload.payload)
            .unwrap();
        assert!(!field.field.contains('#'));
    }
    copy_replica(target.pool(), source.pool()).await;
    apply_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    let restored: String =
        sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'transcript-1'")
            .fetch_one(source.pool())
            .await
            .unwrap();
    assert_eq!(restored, edited_words.to_string());
}

#[tokio::test]
async fn new_table_rows_stay_local_without_starving_compatible_work() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    let target = test_db().await;
    for table in ["daily_notes", "folders", "session_tags", "tags"] {
        for index in 0..70 {
            let sql = format!("INSERT INTO {table} (id, workspace_id) VALUES (?, 'workspace-a')");
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(format!("row-{index}"))
                .execute(source.pool())
                .await
                .unwrap();
        }
    }
    sqlx::query("INSERT INTO sessions (id, workspace_id, title) VALUES ('session-1', 'workspace-a', 'Safe to sync')")
        .execute(source.pool()).await.unwrap();
    assert!(
        has_pending_e2ee_dirty_rows_deferring_active_captures(source.pool(), &workspace_keys)
            .await
            .unwrap()
    );
    let stats = encrypt_e2ee_replica_changes_bounded(source.pool(), &workspace_keys, 1)
        .await
        .unwrap();
    assert!(stats.encrypted_fields > 0);
    assert!(!stats.remaining_replica_changes);
    assert!(
        !has_pending_e2ee_dirty_rows_deferring_active_captures(source.pool(), &workspace_keys)
            .await
            .unwrap()
    );
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_dirty_rows WHERE workspace_id = 'workspace-a'",
    )
    .fetch_one(source.pool())
    .await
    .unwrap();
    assert_eq!(pending, 280, "deferred local changes must be retained");
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT table_name FROM e2ee_local_state WHERE workspace_id = 'workspace-a'",
    )
    .fetch_all(source.pool())
    .await
    .unwrap();
    assert_eq!(tables, vec!["sessions"]);
    copy_replica(source.pool(), target.pool()).await;
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(target.pool())
        .await
        .unwrap();
    assert_eq!(title, "Safe to sync");
}

#[tokio::test]
async fn compatibility_policy_covers_every_domain_table() {
    let db = test_db().await;
    let deferred_tables = ["daily_notes", "folders", "session_tags", "tags"];
    assert!(
        deferred_tables
            .iter()
            .all(|table| E2EE_DOMAIN_TABLES.contains(table))
    );
    let sql = format!(
        "SELECT {E2EE_DIRTY_ROW_WRITE_COMPATIBILITY_PREDICATE}
         FROM (SELECT ? AS table_name, 'workspace-a' AS workspace_id, 'new-row' AS row_id) AS dirty"
    );
    for table in E2EE_DOMAIN_TABLES {
        let eligible: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(table)
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(eligible, !deferred_tables.contains(table), "{table}");
    }
}

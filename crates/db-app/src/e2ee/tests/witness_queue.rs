use super::*;

#[tokio::test]
async fn pending_witness_uploads_respect_record_and_byte_limits() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Bounded witness uploads')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();
    let key = &workspace_keys["workspace-a"];

    let first = pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 1, usize::MAX)
        .await
        .unwrap();
    assert_eq!(first.len(), 1);
    let first_bytes = first[0]
        .payload
        .len()
        .saturating_add(first[0].record_id.len())
        .saturating_add(first[0].payload_hash.len())
        .saturating_add(256);
    let byte_bounded =
        pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 128, first_bytes)
            .await
            .unwrap();
    assert_eq!(byte_bounded, first);
    assert!(matches!(
        pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 128, first_bytes - 1,).await,
        Err(E2eeReplicaError::WitnessUploadTooLarge)
    ));

    acknowledge_e2ee_witness_uploads(db.pool(), key, &byte_bounded)
        .await
        .unwrap();
    let next = pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 1, usize::MAX)
        .await
        .unwrap();
    assert_eq!(next.len(), 1);
    assert_ne!(next[0].record_id, first[0].record_id);
}

#[tokio::test]
async fn pending_witness_uploads_never_scan_all_local_state() {
    let db = test_db().await;
    let explain = format!("EXPLAIN QUERY PLAN {PENDING_E2EE_WITNESS_UPLOADS_SQL}");
    let plan: Vec<(i64, i64, i64, String)> = sqlx::query_as(sqlx::AssertSqlSafe(explain.as_str()))
        .bind("workspace-a")
        .bind(16_i64)
        .fetch_all(db.pool())
        .await
        .unwrap();
    let details = plan
        .into_iter()
        .map(|(_, _, _, detail)| detail)
        .collect::<Vec<_>>();

    assert!(details.iter().any(|detail| {
        detail.contains(
            "SEARCH pending USING COVERING INDEX idx_e2ee_witness_pending_workspace_record",
        )
    }));
    assert!(!details.iter().any(|detail| detail.contains("SCAN local")));
}

#[tokio::test]
async fn local_edits_enqueue_and_acknowledgements_drain_witness_uploads() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Before')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    let initial = pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 128, usize::MAX)
        .await
        .unwrap();
    assert!(!initial.is_empty());
    acknowledge_e2ee_witness_uploads(db.pool(), key, &initial)
        .await
        .unwrap();
    let pending_after_ack: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_witness_pending")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(pending_after_ack, 0);

    sqlx::query("UPDATE sessions SET title = 'After' WHERE id = 'session-1'")
        .execute(db.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    let title_record_id = key.blind_field_id("sessions", "session-1", "title");
    let title_pending: bool = sqlx::query_scalar(
        "SELECT EXISTS(
               SELECT 1 FROM e2ee_witness_pending WHERE record_id = ?
             )",
    )
    .bind(title_record_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(title_pending);
}

#[tokio::test]
async fn stale_witness_ack_keeps_a_newer_local_upload_pending() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'First')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    let initial = pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 128, usize::MAX)
        .await
        .unwrap();
    let title_record_id = key.blind_field_id("sessions", "session-1", "title");
    let stale_title = initial
        .iter()
        .find(|upload| upload.record_id == title_record_id)
        .unwrap()
        .clone();
    acknowledge_e2ee_witness_uploads(db.pool(), key, &initial)
        .await
        .unwrap();

    sqlx::query("UPDATE sessions SET title = 'Second' WHERE id = 'session-1'")
        .execute(db.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();
    acknowledge_e2ee_witness_uploads(db.pool(), key, std::slice::from_ref(&stale_title))
        .await
        .unwrap();

    let pending = pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 128, usize::MAX)
        .await
        .unwrap();
    let current_title = pending
        .iter()
        .find(|upload| upload.record_id == title_record_id)
        .unwrap();
    assert!(current_title.revision > stale_title.revision);
    assert_ne!(current_title.payload, stale_title.payload);

    acknowledge_e2ee_witness_uploads(db.pool(), key, std::slice::from_ref(current_title))
        .await
        .unwrap();
    let title_pending: bool = sqlx::query_scalar(
        "SELECT EXISTS(
               SELECT 1 FROM e2ee_witness_pending WHERE record_id = ?
             )",
    )
    .bind(title_record_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!title_pending);
}

#[tokio::test]
async fn equal_remote_state_does_not_enqueue_a_witness_upload() {
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    let source = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Remote')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    let uploads = pending_e2ee_witness_uploads(source.pool(), "workspace-a", key, 128, usize::MAX)
        .await
        .unwrap();
    let events = uploads
        .iter()
        .enumerate()
        .map(|(index, upload)| E2eeWitnessEvent {
            sequence: u64::try_from(index + 1).unwrap(),
            record_id: upload.record_id.clone(),
            workspace_id: upload.workspace_id.clone(),
            payload_hash: upload.payload_hash.clone(),
            payload: upload.payload.clone(),
        })
        .collect::<Vec<_>>();

    let target = test_db().await;
    copy_replica(source.pool(), target.pool()).await;
    merge_e2ee_witness_events(target.pool(), key, "workspace-a", &events)
        .await
        .unwrap();
    let stats = apply_e2ee_replica_changes_with_witness(target.pool(), &workspace_keys)
        .await
        .unwrap();

    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(target.pool())
        .await
        .unwrap();
    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_witness_pending")
        .fetch_one(target.pool())
        .await
        .unwrap();
    assert!(stats.applied_fields > 0);
    assert_eq!(title, "Remote");
    assert_eq!(pending, 0);
}

#[tokio::test]
async fn deleting_local_state_cascades_to_the_witness_queue() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Pending')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();
    let title_record_id = key.blind_field_id("sessions", "session-1", "title");
    let pending_before: bool = sqlx::query_scalar(
        "SELECT EXISTS(
               SELECT 1 FROM e2ee_witness_pending WHERE record_id = ?
             )",
    )
    .bind(&title_record_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(pending_before);

    sqlx::query("DELETE FROM e2ee_local_state WHERE record_id = ?")
        .bind(&title_record_id)
        .execute(db.pool())
        .await
        .unwrap();

    let pending_after: bool = sqlx::query_scalar(
        "SELECT EXISTS(
               SELECT 1 FROM e2ee_witness_pending WHERE record_id = ?
             )",
    )
    .bind(title_record_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!pending_after);
}

#[tokio::test]
async fn remote_apply_guard_preserves_an_existing_local_dirty_marker() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title, status)
             VALUES ('session-1', 'workspace-a', 'user-a', 'First', 'active')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();

    let target = test_db().await;
    copy_replica(source.pool(), target.pool()).await;
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET title = 'Local' WHERE id = 'session-1'")
        .execute(target.pool())
        .await
        .unwrap();
    let generation_before: i64 = sqlx::query_scalar(
        "SELECT generation FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
               AND table_name = 'sessions'
               AND row_id = 'session-1'",
    )
    .fetch_one(target.pool())
    .await
    .unwrap();

    sqlx::query("UPDATE sessions SET status = 'archived' WHERE id = 'session-1'")
        .execute(source.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    let key = &workspace_keys["workspace-a"];
    let status_record_id = key.blind_field_id("sessions", "session-1", "status");
    let status_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&status_record_id)
            .fetch_one(source.pool())
            .await
            .unwrap();
    sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(status_payload)
        .bind(status_record_id)
        .execute(target.pool())
        .await
        .unwrap();

    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    let row: (String, String) =
        sqlx::query_as("SELECT title, status FROM sessions WHERE id = 'session-1'")
            .fetch_one(target.pool())
            .await
            .unwrap();
    let generation_after: i64 = sqlx::query_scalar(
        "SELECT generation FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
               AND table_name = 'sessions'
               AND row_id = 'session-1'",
    )
    .fetch_one(target.pool())
    .await
    .unwrap();
    assert_eq!(row, ("Local".to_string(), "archived".to_string()));
    assert_eq!(generation_after, generation_before);

    let encrypted = encrypt_e2ee_replica_changes_bounded(target.pool(), &workspace_keys, 64)
        .await
        .unwrap();
    let title_record_id = key.blind_field_id("sessions", "session-1", "title");
    let title_payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
        .bind(&title_record_id)
        .fetch_one(target.pool())
        .await
        .unwrap();
    let title_field = key
        .open_field("workspace-a", &title_record_id, &title_payload)
        .unwrap();
    let remaining_dirty: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
        .fetch_one(target.pool())
        .await
        .unwrap();
    assert!(encrypted.encrypted_fields > 0);
    assert_eq!(title_field.value, json!("Local"));
    assert_eq!(remaining_dirty, 0);
}

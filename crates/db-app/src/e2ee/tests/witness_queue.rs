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

#[tokio::test]
async fn witness_uploads_send_session_metadata_before_bodies() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Title')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_documents (id, workspace_id, session_id, kind, body_format, body)
             VALUES ('session-1', 'workspace-a', 'session-1', 'note', 'text', 'body')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO transcripts (id, workspace_id, owner_user_id, session_id, words_json)
             VALUES ('transcript-1', 'workspace-a', 'user-a', 'session-1', '[]')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    let session_records: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_witness_pending AS pending
         JOIN e2ee_local_state AS local ON local.record_id = pending.record_id
         WHERE local.table_name = 'sessions'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let session_records = usize::try_from(session_records).unwrap();

    // A single batch spanning every priority keeps metadata ahead of bodies.
    let batch = pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 1_000, usize::MAX)
        .await
        .unwrap();
    let mut batch_tables = Vec::with_capacity(batch.len());
    for upload in &batch {
        batch_tables.push(upload_table(db.pool(), &upload.record_id).await);
    }
    assert_metadata_before_bodies(&batch_tables, session_records);

    let mut seen_tables = Vec::new();
    loop {
        let batch = pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 1, usize::MAX)
            .await
            .unwrap();
        let Some(upload) = batch.first() else {
            break;
        };
        seen_tables.push(upload_table(db.pool(), &upload.record_id).await);
        acknowledge_e2ee_witness_uploads(db.pool(), key, &batch)
            .await
            .unwrap();
    }
    assert_eq!(seen_tables, batch_tables);
    assert_metadata_before_bodies(&seen_tables, session_records);
}

async fn upload_table(pool: &SqlitePool, record_id: &str) -> String {
    sqlx::query_scalar("SELECT table_name FROM e2ee_local_state WHERE record_id = ?")
        .bind(record_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn assert_metadata_before_bodies(tables: &[String], session_records: usize) {
    assert!(
        tables[..session_records]
            .iter()
            .all(|table| table == "sessions")
    );
    let document_position = tables.iter().position(|table| table == "session_documents");
    let transcript_position = tables.iter().position(|table| table == "transcripts");
    assert!(document_position.unwrap() < transcript_position.unwrap());
    assert!(
        tables[transcript_position.unwrap()..]
            .iter()
            .all(|table| table == "transcripts")
    );
}

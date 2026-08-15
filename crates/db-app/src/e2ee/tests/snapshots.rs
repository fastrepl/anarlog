use super::*;

#[tokio::test]
async fn rotated_keyring_applies_new_writes_over_an_old_snapshot() {
    let recovery =
        RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc").unwrap();
    let old_key = recovery.workspace_key("workspace-a").unwrap();
    let old_keys = HashMap::from([(
        "workspace-a".to_string(),
        anlg_e2ee::WorkspaceKeyring::new(old_key.clone()),
    )]);
    let source = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Before rotation')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &old_keys)
        .await
        .unwrap();

    let target = test_db().await;
    copy_replica(source.pool(), target.pool()).await;
    apply_e2ee_replica_changes(target.pool(), &old_keys)
        .await
        .unwrap();

    let new_key = anlg_e2ee::WorkspaceKey::generate().unwrap();
    let old_title_id = old_key.blind_field_id("sessions", "session-1", "title");
    let new_title_id = new_key.blind_field_id("sessions", "session-1", "title");
    let mut rotated_keyring = anlg_e2ee::WorkspaceKeyring::new(new_key);
    rotated_keyring.insert_retired(old_key);
    let rotated_keys = HashMap::from([("workspace-a".to_string(), rotated_keyring)]);

    sqlx::query("UPDATE sessions SET title = 'After rotation' WHERE id = 'session-1'")
        .execute(source.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &rotated_keys)
        .await
        .unwrap();
    copy_replica(source.pool(), target.pool()).await;
    let stats = apply_e2ee_replica_changes(target.pool(), &rotated_keys)
        .await
        .unwrap();

    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(target.pool())
        .await
        .unwrap();
    let title_generations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records WHERE id IN (?, ?)")
            .bind(old_title_id)
            .bind(new_title_id)
            .fetch_one(target.pool())
            .await
            .unwrap();
    assert_eq!(title, "After rotation");
    assert_eq!(title_generations, 2);
    assert_eq!(stats.skipped_local_changes, 0);
}

#[tokio::test]
async fn rotated_keyring_authenticates_witness_history_from_a_retired_generation() {
    let db = test_db().await;
    let recovery =
        RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc").unwrap();
    let old_key = recovery.workspace_key("workspace-a").unwrap();
    let sealed = old_key
        .seal_field(
            "workspace-a",
            "sessions",
            "session-1",
            "title",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            1,
            false,
            serde_json::json!("Before rotation"),
        )
        .unwrap();
    let event = E2eeWitnessEvent {
        sequence: 1,
        record_id: sealed.record_id,
        workspace_id: "workspace-a".to_string(),
        payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
        payload: sealed.payload,
    };
    let mut keyring =
        anlg_e2ee::WorkspaceKeyring::new(anlg_e2ee::WorkspaceKey::generate().unwrap());
    keyring.insert_retired(old_key);

    merge_e2ee_witness_events_with_keyring(db.pool(), &keyring, "workspace-a", &[event])
        .await
        .unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM e2ee_witness_records WHERE workspace_id = 'workspace-a'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn fresh_device_does_not_materialize_an_unwitnessed_snapshot() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Archived value')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();

    let fresh = test_db().await;
    copy_replica(source.pool(), fresh.pool()).await;
    let stats = apply_e2ee_replica_changes_with_witness(fresh.pool(), &workspace_keys)
        .await
        .unwrap();

    let materialized: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE id = 'session-1'")
            .fetch_one(fresh.pool())
            .await
            .unwrap();
    assert_eq!(materialized, 0);
    assert!(stats.rejected_unwitnessed > 0);
}

#[tokio::test]
async fn witnessed_snapshot_materializes_and_repairs_the_replica() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Witnessed value')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();

    let encrypted: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, payload FROM e2ee_records WHERE workspace_id = 'workspace-a' ORDER BY id",
    )
    .fetch_all(source.pool())
    .await
    .unwrap();
    let events = encrypted
        .iter()
        .enumerate()
        .map(|(index, (record_id, payload))| E2eeWitnessEvent {
            sequence: u64::try_from(index + 1).unwrap(),
            record_id: record_id.clone(),
            workspace_id: "workspace-a".to_string(),
            payload_hash: anlg_e2ee::payload_hash(payload),
            payload: payload.clone(),
        })
        .collect::<Vec<_>>();

    let fresh = test_db().await;
    copy_replica(source.pool(), fresh.pool()).await;
    merge_e2ee_witness_events(
        fresh.pool(),
        &workspace_keys["workspace-a"],
        "workspace-a",
        &events,
    )
    .await
    .unwrap();
    advance_e2ee_witness_cursor(fresh.pool(), "workspace-a", events.last().unwrap().sequence)
        .await
        .unwrap();
    apply_e2ee_replica_changes_with_witness(fresh.pool(), &workspace_keys)
        .await
        .unwrap();

    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(fresh.pool())
        .await
        .unwrap();
    assert_eq!(title, "Witnessed value");

    sqlx::query("DELETE FROM e2ee_records")
        .execute(fresh.pool())
        .await
        .unwrap();
    apply_e2ee_replica_changes_with_witness(fresh.pool(), &workspace_keys)
        .await
        .unwrap();

    let repaired: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records WHERE workspace_id = 'workspace-a'")
            .fetch_one(fresh.pool())
            .await
            .unwrap();
    assert_eq!(repaired, i64::try_from(events.len()).unwrap());
}

#[tokio::test]
async fn no_op_encryption_and_witness_repair_do_not_touch_replica_rows() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Unchanged')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    let key = &workspace_keys["workspace-a"];
    let title_record_id = key.blind_field_id("sessions", "session-1", "title");
    let title_payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
        .bind(&title_record_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE e2ee_records SET updated_at = 'sentinel' WHERE id = ?")
        .bind(&title_record_id)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
             VALUES ('workspace-a', 'sessions', 'session-1')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let stats = encrypt_e2ee_replica_changes_bounded(db.pool(), &workspace_keys, 64)
        .await
        .unwrap();
    let updated_at: String = sqlx::query_scalar("SELECT updated_at FROM e2ee_records WHERE id = ?")
        .bind(&title_record_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(stats.encrypted_fields, 0);
    assert_eq!(updated_at, "sentinel");

    merge_e2ee_witness_events(
        db.pool(),
        key,
        "workspace-a",
        &[E2eeWitnessEvent {
            sequence: 1,
            record_id: title_record_id.clone(),
            workspace_id: "workspace-a".to_string(),
            payload_hash: anlg_e2ee::payload_hash(&title_payload),
            payload: title_payload,
        }],
    )
    .await
    .unwrap();
    apply_received_e2ee_replica_changes_with_witness(db.pool(), &workspace_keys, true)
        .await
        .unwrap();
    let repaired_updated_at: String =
        sqlx::query_scalar("SELECT updated_at FROM e2ee_records WHERE id = ?")
            .bind(title_record_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(repaired_updated_at, "sentinel");
}

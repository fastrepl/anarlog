use super::*;

#[tokio::test]
async fn rejects_authenticated_payloads_in_the_wrong_workspace() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    let sealed = key
        .seal_field(
            "workspace-a",
            "sessions",
            "session-1",
            ROW_MANIFEST_FIELD,
            "00000000000000000000000000000001",
            1,
            false,
            json!(true),
        )
        .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_records (id, workspace_id, payload) VALUES (?, 'workspace-b', ?)",
    )
    .bind(sealed.record_id)
    .bind(sealed.payload)
    .execute(db.pool())
    .await
    .unwrap();

    let mut wrong_keys = workspace_keys;
    wrong_keys.insert(
        "workspace-b".to_string(),
        RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc")
            .unwrap()
            .workspace_key("workspace-b")
            .unwrap(),
    );
    assert!(
        apply_e2ee_replica_changes(db.pool(), &wrong_keys)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rejects_and_repairs_replayed_older_field_revisions() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
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
    let record_id = workspace_keys["workspace-a"].blind_field_id("sessions", "session-1", "title");
    let old_payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
        .bind(&record_id)
        .fetch_one(db.pool())
        .await
        .unwrap();

    sqlx::query("UPDATE sessions SET title = 'Second' WHERE id = 'session-1'")
        .execute(db.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();
    let current_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&record_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    apply_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();
    sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(old_payload)
        .bind(&record_id)
        .execute(db.pool())
        .await
        .unwrap();

    let stats = apply_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();
    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let repaired_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(record_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(stats.rejected_rollbacks, 1);
    assert_eq!(title, "Second");
    assert_eq!(repaired_payload, current_payload);
}

#[tokio::test]
async fn equal_revision_payloads_converge_and_repair_the_replica() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Base')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    let key = &workspace_keys["workspace-a"];
    let first = key
        .seal_field(
            "workspace-a",
            "sessions",
            "session-1",
            "title",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            2,
            false,
            json!("Device A"),
        )
        .unwrap();
    let second = key
        .seal_field(
            "workspace-a",
            "sessions",
            "session-1",
            "title",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            2,
            false,
            json!("Device B"),
        )
        .unwrap();
    let (winner, winner_title, replay) = (second, "Device B", first);

    sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(&winner.payload)
        .bind(&winner.record_id)
        .execute(db.pool())
        .await
        .unwrap();
    apply_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(&replay.payload)
        .bind(&replay.record_id)
        .execute(db.pool())
        .await
        .unwrap();
    apply_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let canonical_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&winner.record_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(title, winner_title);
    assert_eq!(canonical_payload, winner.payload);

    let third = key
        .seal_field(
            "workspace-a",
            "sessions",
            "session-1",
            "title",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            3,
            false,
            json!("Clone A"),
        )
        .unwrap();
    let fourth = key
        .seal_field(
            "workspace-a",
            "sessions",
            "session-1",
            "title",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            3,
            false,
            json!("Clone B"),
        )
        .unwrap();
    let (winner, winner_title, replay) =
        if anlg_e2ee::payload_hash(&third.payload) > anlg_e2ee::payload_hash(&fourth.payload) {
            (third, "Clone A", fourth)
        } else {
            (fourth, "Clone B", third)
        };

    sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(&winner.payload)
        .bind(&winner.record_id)
        .execute(db.pool())
        .await
        .unwrap();
    apply_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();
    sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(&replay.payload)
        .bind(&replay.record_id)
        .execute(db.pool())
        .await
        .unwrap();
    let stats = apply_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let canonical_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&winner.record_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(stats.rejected_rollbacks, 1);
    assert_eq!(title, winner_title);
    assert_eq!(canonical_payload, winner.payload);
}

#[tokio::test]
async fn writer_ids_are_stable_per_device_and_unique_across_devices() {
    let first = test_db().await;
    sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES ('first', 'workspace-a')")
        .execute(first.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(first.pool(), &keys("workspace-a"))
        .await
        .unwrap();
    let first_writer: String =
        sqlx::query_scalar("SELECT writer_id FROM e2ee_local_device WHERE id = 'local'")
            .fetch_one(first.pool())
            .await
            .unwrap();
    encrypt_e2ee_replica_changes(first.pool(), &keys("workspace-a"))
        .await
        .unwrap();
    let repeated_writer: String =
        sqlx::query_scalar("SELECT writer_id FROM e2ee_local_device WHERE id = 'local'")
            .fetch_one(first.pool())
            .await
            .unwrap();

    let second = test_db().await;
    sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES ('second', 'workspace-a')")
        .execute(second.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(second.pool(), &keys("workspace-a"))
        .await
        .unwrap();
    let second_writer: String =
        sqlx::query_scalar("SELECT writer_id FROM e2ee_local_device WHERE id = 'local'")
            .fetch_one(second.pool())
            .await
            .unwrap();

    assert_eq!(first_writer, repeated_writer);
    assert_ne!(first_writer, second_writer);
    assert_eq!(first_writer.len(), 32);
    assert_eq!(second_writer.len(), 32);
}

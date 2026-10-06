use super::*;

async fn title(pool: &SqlitePool) -> String {
    sqlx::query_scalar("SELECT title FROM sessions WHERE id = 's'")
        .fetch_one(pool)
        .await
        .unwrap()
}

pub(super) async fn accept(
    pool: &SqlitePool,
    keyring: &anlg_e2ee::WorkspaceKeyring,
    batch: &E2eeCloudBatch,
    head: &mut u64,
) -> Vec<E2eeWitnessEvent> {
    let events = batch
        .events
        .iter()
        .map(|e| {
            *head += 1;
            E2eeWitnessEvent {
                sequence: *head,
                workspace_id: "workspace-a".into(),
                record_id: e.record_id.clone(),
                payload_hash: e.payload_hash.clone(),
                payload: e.payload.clone(),
            }
        })
        .collect::<Vec<_>>();
    merge_e2ee_witness_events_with_keyring(pool, keyring, "workspace-a", &events)
        .await
        .unwrap();
    acknowledge_e2ee_cloud_batch(pool, "workspace-a", batch)
        .await
        .unwrap();
    advance_e2ee_witness_cursor(pool, "workspace-a", *head)
        .await
        .unwrap();
    events
}

#[tokio::test]
async fn cloud_order_survives_lower_client_revisions_and_pending_edits_survive_late_receipts() {
    let db = test_db().await;
    let fresh = test_db().await;
    let keys = keys("workspace-a");
    let keyring = &keys["workspace-a"];
    for pool in [db.pool(), fresh.pool()] {
        configure_e2ee_cloud_authority(pool, "workspace-a", Some(0))
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES('s','workspace-a','u','Initial')").execute(db.pool()).await.unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();
    let first = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
        .unwrap();
    let mut head = 0;
    let events = accept(db.pool(), keyring, &first, &mut head).await;
    merge_e2ee_witness_events_with_keyring(fresh.pool(), keyring, "workspace-a", &events)
        .await
        .unwrap();
    apply_received_e2ee_replica_changes_with_witness(fresh.pool(), &keys, true)
        .await
        .unwrap();
    assert_eq!(title(fresh.pool()).await, "Initial");

    sqlx::query("UPDATE sessions SET title = 'Offline' WHERE id = 's'")
        .execute(db.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();
    let pending = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
        .unwrap();
    // Freezing a retry is durable; reloading it cannot change its id or payload.
    let retry = pending_e2ee_cloud_batch(db.pool(), "workspace-a", true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(&pending).unwrap(),
        serde_json::to_value(retry).unwrap()
    );
    sqlx::query("UPDATE sessions SET title = 'Newer offline' WHERE id = 's'")
        .execute(db.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();

    let sealed = keyring
        .active()
        .seal_field_at(
            "workspace-a",
            "sessions",
            "s",
            "title",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            1,
            false,
            Some(1),
            json!("Cloud"),
        )
        .unwrap();
    head += 1;
    let cloud = E2eeWitnessEvent {
        sequence: head,
        workspace_id: "workspace-a".into(),
        record_id: sealed.record_id,
        payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
        payload: sealed.payload,
    };
    for pool in [db.pool(), fresh.pool()] {
        merge_e2ee_witness_events_with_keyring(
            pool,
            keyring,
            "workspace-a",
            std::slice::from_ref(&cloud),
        )
        .await
        .unwrap();
        apply_received_e2ee_replica_changes_with_witness(pool, &keys, true)
            .await
            .unwrap();
    }
    assert_eq!(title(db.pool()).await, "Newer offline");
    assert_eq!(title(fresh.pool()).await, "Cloud");
    accept(db.pool(), keyring, &pending, &mut head).await;
    assert!(
        pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
            .await
            .unwrap()
            .is_some()
    );
    let latest = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
        .unwrap();
    let events = accept(db.pool(), keyring, &latest, &mut head).await;
    merge_e2ee_witness_events_with_keyring(fresh.pool(), keyring, "workspace-a", &events)
        .await
        .unwrap();
    for pool in [db.pool(), fresh.pool()] {
        apply_received_e2ee_replica_changes_with_witness(pool, &keys, true)
            .await
            .unwrap();
        assert_eq!(title(pool).await, "Newer offline");
    }
    assert!(
        pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
            .await
            .unwrap()
            .is_none()
    );
    let mut restored = cloud;
    restored.sequence = head + 1;
    set_e2ee_cloud_pull_in_progress(db.pool(), "workspace-a", true)
        .await
        .unwrap();
    merge_e2ee_witness_events_with_keyring(db.pool(), keyring, "workspace-a", &[restored])
        .await
        .unwrap();
    apply_received_e2ee_replica_changes_with_witness(db.pool(), &keys, true)
        .await
        .unwrap();
    assert_eq!(title(db.pool()).await, "Newer offline");
    set_e2ee_cloud_pull_in_progress(db.pool(), "workspace-a", false)
        .await
        .unwrap();
    apply_received_e2ee_replica_changes_with_witness(db.pool(), &keys, true)
        .await
        .unwrap();
    assert_eq!(title(db.pool()).await, "Cloud");
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();
    assert!(
        pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        configure_e2ee_cloud_authority(db.pool(), "workspace-a", None)
            .await
            .is_err()
    );
}

fn document(a: &str, b: &str) -> String {
    json!({"type":"doc", "content": ([a,b].map(|text| json!({"type":"paragraph","content":[{"type":"text","text":text}]})))}).to_string()
}

#[tokio::test]
async fn cloud_rebase_merges_disjoint_document_edits_before_freezing_the_upload() {
    let db = test_db().await;
    let keys = keys("workspace-a");
    let keyring = &keys["workspace-a"];
    configure_e2ee_cloud_authority(db.pool(), "workspace-a", Some(0))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions(id,workspace_id,owner_user_id) VALUES('s','workspace-a','u')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO session_documents(id,workspace_id,session_id,kind,body_format,body) VALUES('s','workspace-a','s','note','prosemirror_json',?)")
        .bind(document("first", "second")).execute(db.pool()).await.unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();
    let mut head = 0;
    while let Some(batch) = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
    {
        accept(db.pool(), keyring, &batch, &mut head).await;
    }
    sqlx::query("UPDATE session_documents SET body = ? WHERE id = 's'")
        .bind(document("local", "second"))
        .execute(db.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();
    let sealed = keyring
        .active()
        .seal_field_at(
            "workspace-a",
            "session_documents",
            "s",
            "body",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            2,
            false,
            Some(1),
            json!(document("first", "cloud")),
        )
        .unwrap();
    head += 1;
    merge_e2ee_witness_events_with_keyring(
        db.pool(),
        keyring,
        "workspace-a",
        &[E2eeWitnessEvent {
            sequence: head,
            workspace_id: "workspace-a".into(),
            record_id: sealed.record_id,
            payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
            payload: sealed.payload,
        }],
    )
    .await
    .unwrap();
    // An edit typed after encryption also belongs to the durable pending overlay.
    sqlx::query("UPDATE session_documents SET body = ? WHERE id = 's'")
        .bind(document("latest local", "second"))
        .execute(db.pool())
        .await
        .unwrap();
    rebase_e2ee_cloud_documents(db.pool(), "workspace-a", keyring)
        .await
        .unwrap();
    let current: String = sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 's'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&current).unwrap(),
        serde_json::from_str::<Value>(&document("latest local", "cloud")).unwrap()
    );
    let batch = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
        .unwrap();
    let field = batch
        .events
        .iter()
        .filter_map(|e| {
            keyring
                .open_field("workspace-a", &e.record_id, &e.payload)
                .ok()
        })
        .find(|f| f.field == "body")
        .unwrap();
    assert_eq!(field.value, json!(current));
    assert!(
        list_e2ee_field_conflicts(db.pool(), "session_documents", "s", false)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn accepting_an_offline_title_edit_does_not_resurrect_a_cloud_tombstone() {
    let db = test_db().await;
    let keys = keys("workspace-a");
    let keyring = &keys["workspace-a"];
    configure_e2ee_cloud_authority(db.pool(), "workspace-a", Some(0))
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions(id,workspace_id,owner_user_id,title) VALUES('s','workspace-a','u','Initial')").execute(db.pool()).await.unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();
    let mut head = 0;
    let first = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
        .unwrap();
    accept(db.pool(), keyring, &first, &mut head).await;
    sqlx::query("UPDATE sessions SET title = 'Offline edit' WHERE id = 's'")
        .execute(db.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();
    let tombstone = "2026-10-05T00:00:00Z";
    let sealed = keyring
        .active()
        .seal_field_at(
            "workspace-a",
            "sessions",
            "s",
            "deleted_at",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            1,
            false,
            Some(1),
            json!(tombstone),
        )
        .unwrap();
    head += 1;
    merge_e2ee_witness_events_with_keyring(
        db.pool(),
        keyring,
        "workspace-a",
        &[E2eeWitnessEvent {
            sequence: head,
            workspace_id: "workspace-a".into(),
            record_id: sealed.record_id,
            payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
            payload: sealed.payload,
        }],
    )
    .await
    .unwrap();
    apply_received_e2ee_replica_changes_with_witness(db.pool(), &keys, true)
        .await
        .unwrap();
    let pending = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
        .unwrap();
    accept(db.pool(), keyring, &pending, &mut head).await;
    apply_received_e2ee_replica_changes_with_witness(db.pool(), &keys, true)
        .await
        .unwrap();
    let deleted: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM sessions WHERE id = 's'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(deleted.as_deref(), Some(tombstone));
    assert_eq!(title(db.pool()).await, "Offline edit");
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();
    assert!(
        pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn replaying_cloud_history_retires_an_edit_from_a_backup_taken_before_batch_creation() {
    let db = test_db().await;
    let keys = keys("workspace-a");
    let keyring = &keys["workspace-a"];
    configure_e2ee_cloud_authority(db.pool(), "workspace-a", Some(0))
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions(id,workspace_id,owner_user_id,title) VALUES('s','workspace-a','u','Already accepted')").execute(db.pool()).await.unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();
    let batch = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
        .unwrap();
    // The restored outbox has ciphertext but no persisted upload identity.
    reject_e2ee_cloud_batch(db.pool(), "workspace-a", &batch.mutation_id)
        .await
        .unwrap();
    let events = batch
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| E2eeWitnessEvent {
            sequence: i as u64 + 1,
            workspace_id: "workspace-a".into(),
            record_id: e.record_id.clone(),
            payload_hash: e.payload_hash.clone(),
            payload: e.payload.clone(),
        })
        .collect::<Vec<_>>();
    merge_e2ee_witness_events_with_keyring(db.pool(), keyring, "workspace-a", &events)
        .await
        .unwrap();
    assert!(
        pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
            .await
            .unwrap()
            .is_none()
    );
    let sealed = keyring
        .active()
        .seal_field_at(
            "workspace-a",
            "sessions",
            "s",
            "title",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            1,
            false,
            Some(1),
            json!("Later cloud edit"),
        )
        .unwrap();
    merge_e2ee_witness_events_with_keyring(
        db.pool(),
        keyring,
        "workspace-a",
        &[E2eeWitnessEvent {
            sequence: events.len() as u64 + 1,
            workspace_id: "workspace-a".into(),
            record_id: sealed.record_id,
            payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
            payload: sealed.payload,
        }],
    )
    .await
    .unwrap();
    apply_received_e2ee_replica_changes_with_witness(db.pool(), &keys, true)
        .await
        .unwrap();
    assert_eq!(title(db.pool()).await, "Later cloud edit");
}

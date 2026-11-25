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

#[tokio::test]
async fn idle_cloud_authority_calls_do_not_leave_open_transactions() {
    use sqlx::Connection;

    let db = test_db().await;
    let keys = keys("workspace-a");
    let keyring = &keys["workspace-a"];

    configure_e2ee_cloud_authority(db.pool(), "workspace-a", Some(0))
        .await
        .unwrap();
    configure_e2ee_cloud_authority(db.pool(), "workspace-a", Some(0))
        .await
        .unwrap();
    assert!(
        configure_e2ee_cloud_authority(db.pool(), "workspace-a", Some(1))
            .await
            .is_err()
    );
    assert!(
        pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
            .await
            .unwrap()
            .is_none()
    );
    rebase_e2ee_cloud_documents(db.pool(), "workspace-a", keyring)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES('s', 'workspace-a', 'u', 'Initial')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();
    let batch = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
        .unwrap();
    let cached = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cached.mutation_id, batch.mutation_id);

    let conn = db.pool().acquire().await.unwrap();
    assert!(!conn.is_in_transaction());
    conn.close().await.unwrap();
}

#[tokio::test]
async fn multi_row_cloud_batch_packs_small_rows_atomically() {
    let db = test_db().await;
    let keys = keys("workspace-a");
    let keyring = &keys["workspace-a"];
    configure_e2ee_cloud_authority(db.pool(), "workspace-a", Some(0))
        .await
        .unwrap();
    for id in ["s1", "s2", "s3"] {
        sqlx::query(
            "INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES(?, 'workspace-a', 'u', ?)",
        )
        .bind(id)
        .bind(format!("Session {id}"))
        .execute(db.pool())
        .await
        .unwrap();
    }
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();

    let batch = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
        .expect("packed cloud batch");
    let row_ids: std::collections::HashSet<_> = batch
        .events
        .iter()
        .map(|event| {
            keyring
                .open_field("workspace-a", &event.record_id, &event.payload)
                .unwrap()
                .row_id
        })
        .collect();
    assert_eq!(row_ids.len(), 3);

    let mut head = 0;
    accept(db.pool(), keyring, &batch, &mut head).await;
    assert!(
        pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn cloud_batch_never_splits_single_large_row_over_packed_cap() {
    let db = test_db().await;
    let keys = keys("workspace-a");
    let keyring = &keys["workspace-a"];
    configure_e2ee_cloud_authority(db.pool(), "workspace-a", Some(0))
        .await
        .unwrap();

    // 'humans' sorts before 'sessions', and 'transcripts' sorts after 'sessions'.
    for i in 0..260 {
        sqlx::query(
            "INSERT INTO e2ee_cloud_outbox(record_id, workspace_id, table_name, row_id, payload_hash, payload)
             VALUES (?, 'workspace-a', 'humans', 'h-large', ?, 'payload')",
        )
        .bind(format!("h-rec-{i:04}"))
        .bind(format!("hash-{i:04}"))
        .execute(db.pool())
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO e2ee_cloud_outbox(record_id, workspace_id, table_name, row_id, payload_hash, payload)
         VALUES ('s-rec-0', 'workspace-a', 'sessions', 's-small', 'hash-s0', 'payload')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    for i in 0..260 {
        sqlx::query(
            "INSERT INTO e2ee_cloud_outbox(record_id, workspace_id, table_name, row_id, payload_hash, payload)
             VALUES (?, 'workspace-a', 'transcripts', 't-large', ?, 'payload')",
        )
        .bind(format!("t-rec-{i:04}"))
        .bind(format!("thash-{i:04}"))
        .execute(db.pool())
        .await
        .unwrap();
    }

    let b1 = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(b1.events.len(), 260);
    assert!(b1.events.iter().all(|e| e.record_id.starts_with("h-rec-")));
    acknowledge_e2ee_cloud_batch(db.pool(), "workspace-a", &b1)
        .await
        .unwrap();

    let b2 = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(b2.events.len(), 1);
    assert_eq!(b2.events[0].record_id, "s-rec-0");
    acknowledge_e2ee_cloud_batch(db.pool(), "workspace-a", &b2)
        .await
        .unwrap();

    let b3 = pending_e2ee_cloud_batch(db.pool(), "workspace-a", false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(b3.events.len(), 260);
    assert!(b3.events.iter().all(|e| e.record_id.starts_with("t-rec-")));
    let _ = keyring;
}

#[tokio::test]
async fn rebase_e2ee_cloud_documents_merges_multiple_pending_documents() {
    let db = test_db().await;
    let keys = keys("workspace-a");
    let keyring = &keys["workspace-a"];
    configure_e2ee_cloud_authority(db.pool(), "workspace-a", Some(0))
        .await
        .unwrap();
    for id in ["d1", "d2"] {
        sqlx::query(
            "INSERT INTO sessions(id,workspace_id,owner_user_id) VALUES(?,'workspace-a','u')",
        )
        .bind(id)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO session_documents(id,workspace_id,session_id,kind,body_format,body) VALUES(?,'workspace-a',?,'note','prosemirror_json',?)")
            .bind(id)
            .bind(id)
            .bind(document("first", "second"))
            .execute(db.pool())
            .await
            .unwrap();
    }
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

    for id in ["d1", "d2"] {
        sqlx::query("UPDATE session_documents SET body = ? WHERE id = ?")
            .bind(document("local", "second"))
            .bind(id)
            .execute(db.pool())
            .await
            .unwrap();
    }
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();

    let mut remote_events = Vec::new();
    for id in ["d1", "d2"] {
        let sealed = keyring
            .active()
            .seal_field_at(
                "workspace-a",
                "session_documents",
                id,
                "body",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                2,
                false,
                Some(1),
                json!(document("first", "cloud")),
            )
            .unwrap();
        head += 1;
        remote_events.push(E2eeWitnessEvent {
            sequence: head,
            workspace_id: "workspace-a".into(),
            record_id: sealed.record_id,
            payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
            payload: sealed.payload,
        });
    }
    merge_e2ee_witness_events_with_keyring(db.pool(), keyring, "workspace-a", &remote_events)
        .await
        .unwrap();

    rebase_e2ee_cloud_documents(db.pool(), "workspace-a", keyring)
        .await
        .unwrap();

    for id in ["d1", "d2"] {
        let current: String = sqlx::query_scalar("SELECT body FROM session_documents WHERE id = ?")
            .bind(id)
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&current).unwrap(),
            serde_json::from_str::<Value>(&document("local", "cloud")).unwrap()
        );
    }
}

#[tokio::test]
async fn preserve_e2ee_cloud_conflicts_preflight_avoids_write_lock_when_clean() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preflight.db");
    let db = anlg_db_core::Db::connect_local_plain(&path).await.unwrap();
    crate::prepare_schema(&db).await.unwrap();
    let locker = anlg_db_core::Db::connect_local_plain(&path).await.unwrap();
    let keys = keys("workspace-a");
    let keyring = &keys["workspace-a"];
    let clean_batch = E2eeCloudBatch {
        mutation_id: "clean-batch".into(),
        base_sequence: 0,
        initialize: false,
        events: vec![E2eeCloudEvent {
            record_id: "nonexistent-record".into(),
            payload_hash: "hash".into(),
            payload: "payload".into(),
        }],
    };
    let mut writer = locker.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query("INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES('held','workspace-a','u','Held')")
        .execute(&mut *writer)
        .await
        .unwrap();

    tokio::time::timeout(
        std::time::Duration::from_millis(200),
        preserve_e2ee_cloud_conflicts(db.pool(), "workspace-a", keyring, &clean_batch),
    )
    .await
    .expect("clean preflight should not block on BEGIN IMMEDIATE")
    .unwrap();

    writer.rollback().await.unwrap();
}

#[tokio::test]
async fn null_cloud_authority_boundary_keeps_revision_tie_break_on_page_merge() {
    let db = test_db().await;
    let keys = keys("workspace-a");
    let keyring = &keys["workspace-a"];
    configure_e2ee_cloud_authority(db.pool(), "workspace-a", None)
        .await
        .unwrap();

    let manifest = keyring
        .active()
        .seal_field_at(
            "workspace-a",
            "sessions",
            "s",
            ROW_MANIFEST_FIELD,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            1,
            false,
            Some(10),
            json!(true),
        )
        .unwrap();
    let high_rev = keyring
        .active()
        .seal_field_at(
            "workspace-a",
            "sessions",
            "s",
            "title",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            8,
            false,
            Some(10),
            json!("Higher Revision"),
        )
        .unwrap();
    let low_rev_later_seq = keyring
        .active()
        .seal_field_at(
            "workspace-a",
            "sessions",
            "s",
            "title",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            7,
            false,
            Some(9),
            json!("Lower Revision"),
        )
        .unwrap();

    merge_e2ee_witness_events_with_keyring(
        db.pool(),
        keyring,
        "workspace-a",
        &[
            E2eeWitnessEvent {
                sequence: 9,
                workspace_id: "workspace-a".into(),
                record_id: manifest.record_id,
                payload_hash: anlg_e2ee::payload_hash(&manifest.payload),
                payload: manifest.payload,
            },
            E2eeWitnessEvent {
                sequence: 10,
                workspace_id: "workspace-a".into(),
                record_id: high_rev.record_id.clone(),
                payload_hash: anlg_e2ee::payload_hash(&high_rev.payload),
                payload: high_rev.payload,
            },
            E2eeWitnessEvent {
                sequence: 11,
                workspace_id: "workspace-a".into(),
                record_id: low_rev_later_seq.record_id,
                payload_hash: anlg_e2ee::payload_hash(&low_rev_later_seq.payload),
                payload: low_rev_later_seq.payload,
            },
        ],
    )
    .await
    .unwrap();

    apply_received_e2ee_replica_changes_with_witness(db.pool(), &keys, true)
        .await
        .unwrap();
    assert_eq!(title(db.pool()).await, "Higher Revision");
}

#[tokio::test]
async fn export_snapshot_defers_when_witness_records_exceed_cursor() {
    let db = test_db().await;
    let keys = keys("workspace-a");
    let keyring = &keys["workspace-a"];
    let e1 = keyring
        .active()
        .seal_field_at(
            "workspace-a",
            "sessions",
            "s1",
            "title",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            1,
            false,
            Some(1),
            json!("First"),
        )
        .unwrap();
    merge_e2ee_witness_events_with_keyring(
        db.pool(),
        keyring,
        "workspace-a",
        &[E2eeWitnessEvent {
            sequence: 5,
            workspace_id: "workspace-a".into(),
            record_id: e1.record_id,
            payload_hash: anlg_e2ee::payload_hash(&e1.payload),
            payload: e1.payload,
        }],
    )
    .await
    .unwrap();
    advance_e2ee_witness_cursor(db.pool(), "workspace-a", 4)
        .await
        .unwrap();
    assert!(
        export_e2ee_workspace_snapshot(db.pool(), "workspace-a")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn import_snapshot_does_not_downgrade_newer_retained_witness_or_drop_repair() {
    let db = test_db().await;
    let keys = keys("workspace-a");
    let keyring = &keys["workspace-a"];
    configure_e2ee_cloud_authority(db.pool(), "workspace-a", Some(0))
        .await
        .unwrap();
    let older = keyring
        .active()
        .seal_field_at(
            "workspace-a",
            "sessions",
            "s1",
            "title",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            1,
            false,
            Some(1),
            json!("Older Snapshot Title"),
        )
        .unwrap();
    let newer = keyring
        .active()
        .seal_field_at(
            "workspace-a",
            "sessions",
            "s1",
            "title",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            2,
            false,
            Some(2),
            json!("Newer Witness Title"),
        )
        .unwrap();
    let newer_hash = anlg_e2ee::payload_hash(&newer.payload);
    // Simulate a prior cancelled merge that left sequence 100 in witness tables while cursor < 90.
    merge_e2ee_witness_events_with_keyring(
        db.pool(),
        keyring,
        "workspace-a",
        &[E2eeWitnessEvent {
            sequence: 100,
            workspace_id: "workspace-a".into(),
            record_id: newer.record_id.clone(),
            payload_hash: newer_hash.clone(),
            payload: newer.payload.clone(),
        }],
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT OR REPLACE INTO e2ee_witness_repair_pending(record_id, workspace_id) VALUES (?, 'workspace-a')",
    )
    .bind(&newer.record_id)
    .execute(db.pool())
    .await
    .unwrap();
    advance_e2ee_witness_cursor(db.pool(), "workspace-a", 10)
        .await
        .unwrap();

    let snapshot = anlg_e2ee::WorkspaceSnapshotPayload {
        workspace_id: "workspace-a".to_string(),
        snapshot_sequence: 90,
        cloud_authority_after: Some(0),
        records: vec![anlg_e2ee::SnapshotRecord {
            record_id: older.record_id.clone(),
            revision: 1,
            writer_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            payload_hash: anlg_e2ee::payload_hash(&older.payload),
            payload: older.payload.clone(),
            sequence: 80,
        }],
    };
    import_e2ee_workspace_snapshot(db.pool(), keyring, &snapshot)
        .await
        .unwrap();

    let retained_witness_hash: String = sqlx::query_scalar(
        "SELECT payload_hash FROM e2ee_witness_records WHERE workspace_id = 'workspace-a' AND record_id = ?",
    )
    .bind(&older.record_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(retained_witness_hash, newer_hash);
    let replica_before_repair: Option<String> =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&older.record_id)
            .fetch_optional(db.pool())
            .await
            .unwrap();
    assert_eq!(replica_before_repair, None);
    let repair_still_pending: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM e2ee_witness_repair_pending WHERE record_id = ?)",
    )
    .bind(&older.record_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(repair_still_pending);

    apply_received_e2ee_replica_changes_with_witness(db.pool(), &keys, true)
        .await
        .unwrap();
    let stored_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&older.record_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(stored_payload, newer.payload);
}

#[tokio::test]
async fn rebase_e2ee_cloud_documents_handles_multiple_tx_batches() {
    let db = test_db().await;
    let keys = keys("workspace-a");
    let keyring = &keys["workspace-a"];
    configure_e2ee_cloud_authority(db.pool(), "workspace-a", Some(0))
        .await
        .unwrap();
    let ids: Vec<String> = (0..10).map(|i| format!("doc-{i:02}")).collect();
    for id in &ids {
        sqlx::query(
            "INSERT INTO sessions(id,workspace_id,owner_user_id) VALUES(?,'workspace-a','u')",
        )
        .bind(id)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO session_documents(id,workspace_id,session_id,kind,body_format,body) VALUES(?,'workspace-a',?,'note','prosemirror_json',?)")
            .bind(id)
            .bind(id)
            .bind(document("first", "second"))
            .execute(db.pool())
            .await
            .unwrap();
    }
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
    for id in &ids {
        sqlx::query("UPDATE session_documents SET body = ? WHERE id = ?")
            .bind(document("local", "second"))
            .bind(id)
            .execute(db.pool())
            .await
            .unwrap();
    }
    encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();

    let mut remote_events = Vec::new();
    for id in &ids {
        let sealed = keyring
            .active()
            .seal_field_at(
                "workspace-a",
                "session_documents",
                id,
                "body",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                2,
                false,
                Some(1),
                json!(document("first", "cloud")),
            )
            .unwrap();
        head += 1;
        remote_events.push(E2eeWitnessEvent {
            sequence: head,
            workspace_id: "workspace-a".into(),
            record_id: sealed.record_id,
            payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
            payload: sealed.payload,
        });
    }
    merge_e2ee_witness_events_with_keyring(db.pool(), keyring, "workspace-a", &remote_events)
        .await
        .unwrap();

    rebase_e2ee_cloud_documents(db.pool(), "workspace-a", keyring)
        .await
        .unwrap();
    for id in &ids {
        let current: String = sqlx::query_scalar("SELECT body FROM session_documents WHERE id = ?")
            .bind(id)
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&current).unwrap(),
            serde_json::from_str::<Value>(&document("local", "cloud")).unwrap()
        );
    }
}

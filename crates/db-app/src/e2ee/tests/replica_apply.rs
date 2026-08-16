use super::*;

#[tokio::test]
async fn empty_replica_placeholders_wait_for_ciphertext() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session-1', 'workspace-a', 'user-a', 'Ready')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    let record_id = workspace_keys["workspace-a"].blind_field_id("sessions", "session-1", "title");
    let payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
        .bind(&record_id)
        .fetch_one(source.pool())
        .await
        .unwrap();

    let target = test_db().await;
    sqlx::query("INSERT INTO e2ee_records (id, workspace_id) VALUES (?, 'workspace-a')")
        .bind(&record_id)
        .execute(target.pool())
        .await
        .unwrap();

    let stats = apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    assert_eq!(stats, E2eeReplicaStats::default());
    let repair = repair_e2ee_replica_from_witness_bounded(
        target.pool(),
        &workspace_keys,
        true,
        64,
        usize::MAX,
    )
    .await
    .unwrap();
    assert_eq!(repair.repaired_records, 0);
    assert!(!repair.remaining);

    sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(&payload)
        .bind(&record_id)
        .execute(target.pool())
        .await
        .unwrap();
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    let payload_hash: String = sqlx::query_scalar(
        "SELECT payload_hash FROM e2ee_replica_payload_hashes WHERE record_id = ?",
    )
    .bind(&record_id)
    .fetch_one(target.pool())
    .await
    .unwrap();
    assert_eq!(payload_hash, anlg_e2ee::payload_hash(&payload));
}

#[tokio::test]
async fn field_chunk_before_manifest_waits_then_applies_without_echo() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Chunked')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    let key = &workspace_keys["workspace-a"];
    let title_id = key.blind_field_id("sessions", "session-1", "title");
    let manifest_id = key.blind_field_id("sessions", "session-1", ROW_MANIFEST_FIELD);
    let title_payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
        .bind(&title_id)
        .fetch_one(source.pool())
        .await
        .unwrap();
    let manifest_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&manifest_id)
            .fetch_one(source.pool())
            .await
            .unwrap();

    let target = test_db().await;
    sqlx::query(
        "INSERT INTO e2ee_records (id, workspace_id, payload)
             VALUES (?, 'workspace-a', ?)",
    )
    .bind(&title_id)
    .bind(&title_payload)
    .execute(target.pool())
    .await
    .unwrap();
    let before_apply: (String, String) = sqlx::query_as(
        "SELECT replica_hash.payload_hash, replica.updated_at
         FROM e2ee_records AS replica
         JOIN e2ee_replica_payload_hashes AS replica_hash
           ON replica_hash.record_id = replica.id
         WHERE replica.id = ?",
    )
    .bind(&title_id)
    .fetch_one(target.pool())
    .await
    .unwrap();
    assert!(before_apply.0.is_empty());
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    let after_apply: (String, String) = sqlx::query_as(
        "SELECT replica_hash.payload_hash, replica.updated_at
         FROM e2ee_records AS replica
         JOIN e2ee_replica_payload_hashes AS replica_hash
           ON replica_hash.record_id = replica.id
         WHERE replica.id = ?",
    )
    .bind(&title_id)
    .fetch_one(target.pool())
    .await
    .unwrap();
    assert_eq!(after_apply.0, anlg_e2ee::payload_hash(&title_payload));
    assert_eq!(after_apply.1, before_apply.1);
    let before_manifest: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE id = 'session-1'")
            .fetch_one(target.pool())
            .await
            .unwrap();
    assert_eq!(before_manifest, 0);

    sqlx::query(
        "INSERT INTO e2ee_records (id, workspace_id, payload)
             VALUES (?, 'workspace-a', ?)",
    )
    .bind(manifest_id)
    .bind(manifest_payload)
    .execute(target.pool())
    .await
    .unwrap();
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(target.pool())
        .await
        .unwrap();
    let dirty_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
        .fetch_one(target.pool())
        .await
        .unwrap();
    let outbound = encrypt_e2ee_replica_changes_bounded(target.pool(), &workspace_keys, 64)
        .await
        .unwrap();
    assert_eq!(title, "Chunked");
    assert_eq!(dirty_count, 0);
    assert_eq!(outbound.encrypted_fields, 0);
}

#[tokio::test]
async fn deleted_replica_records_clear_their_pending_apply_entries() {
    let workspace_keys = keys("workspace-a");
    let target = test_db().await;
    sqlx::query(
        "INSERT INTO e2ee_records (id, workspace_id, payload)
             VALUES ('deleted-record', 'workspace-a', 'payload')",
    )
    .execute(target.pool())
    .await
    .unwrap();
    sqlx::query("DELETE FROM e2ee_records WHERE id = 'deleted-record'")
        .execute(target.pool())
        .await
        .unwrap();

    let stats = apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_replica_pending WHERE record_id = 'deleted-record'",
    )
    .fetch_one(target.pool())
    .await
    .unwrap();

    assert_eq!(pending, 0);
    assert!(!stats.remaining_replica_changes);
}

#[tokio::test]
async fn witness_repairs_missing_records_only_after_snapshot_completion() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Witness recovery')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    let encrypted: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, payload FROM e2ee_records
             WHERE workspace_id = 'workspace-a'
             ORDER BY id",
    )
    .fetch_all(source.pool())
    .await
    .unwrap();
    let events = encrypted
        .into_iter()
        .enumerate()
        .map(|(index, (record_id, payload))| E2eeWitnessEvent {
            sequence: u64::try_from(index + 1).unwrap(),
            record_id,
            workspace_id: "workspace-a".to_string(),
            payload_hash: anlg_e2ee::payload_hash(&payload),
            payload,
        })
        .collect::<Vec<_>>();

    let target = test_db().await;
    merge_e2ee_witness_events(
        target.pool(),
        &workspace_keys["workspace-a"],
        "workspace-a",
        &events,
    )
    .await
    .unwrap();
    apply_received_e2ee_replica_changes_with_witness(target.pool(), &workspace_keys, false)
        .await
        .unwrap();
    let partial_replica_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records")
        .fetch_one(target.pool())
        .await
        .unwrap();
    let partial_session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(target.pool())
        .await
        .unwrap();
    assert_eq!(partial_replica_count, 0);
    assert_eq!(partial_session_count, 0);

    apply_received_e2ee_replica_changes_with_witness(target.pool(), &workspace_keys, true)
        .await
        .unwrap();
    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(target.pool())
        .await
        .unwrap();
    assert_eq!(title, "Witness recovery");
}

#[tokio::test]
async fn witness_repairs_respect_record_and_byte_limits() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Bounded witness recovery')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    let encrypted: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, payload FROM e2ee_records
             WHERE workspace_id = 'workspace-a'
             ORDER BY id",
    )
    .fetch_all(source.pool())
    .await
    .unwrap();
    assert!(encrypted.len() > 2);
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

    let target = test_db().await;
    merge_e2ee_witness_events(
        target.pool(),
        &workspace_keys["workspace-a"],
        "workspace-a",
        &events,
    )
    .await
    .unwrap();

    let error =
        repair_e2ee_replica_from_witness_bounded(target.pool(), &workspace_keys, true, 2, 1)
            .await
            .unwrap_err();
    assert!(matches!(error, E2eeReplicaError::WitnessRepairTooLarge));
    let empty_replica: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records")
        .fetch_one(target.pool())
        .await
        .unwrap();
    assert_eq!(empty_replica, 0);

    let first = repair_e2ee_replica_from_witness_bounded(
        target.pool(),
        &workspace_keys,
        true,
        2,
        usize::MAX,
    )
    .await
    .unwrap();
    assert_eq!(first.repaired_records, 2);
    assert!(first.remaining);

    let mut remaining = first.remaining;
    while remaining {
        let outcome = repair_e2ee_replica_from_witness_bounded(
            target.pool(),
            &workspace_keys,
            true,
            2,
            usize::MAX,
        )
        .await
        .unwrap();
        assert!(outcome.repaired_records <= 2);
        remaining = outcome.remaining;
    }

    let repaired: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records")
        .fetch_one(target.pool())
        .await
        .unwrap();
    assert_eq!(repaired, i64::try_from(events.len()).unwrap());
    assert!(
        !has_pending_e2ee_witness_repairs(target.pool(), &workspace_keys, true)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn completed_snapshots_repair_large_witness_sets_in_bounded_cycles() {
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    let events = (0..130)
        .map(|index| {
            let row_id = format!("session-{index:03}");
            let sealed = key
                .seal_field(
                    "workspace-a",
                    "sessions",
                    &row_id,
                    ROW_MANIFEST_FIELD,
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    1,
                    false,
                    json!(true),
                )
                .unwrap();
            E2eeWitnessEvent {
                sequence: u64::try_from(index + 1).unwrap(),
                record_id: sealed.record_id,
                workspace_id: "workspace-a".to_string(),
                payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
                payload: sealed.payload,
            }
        })
        .collect::<Vec<_>>();

    let target = test_db().await;
    merge_e2ee_witness_events(target.pool(), key, "workspace-a", &events)
        .await
        .unwrap();
    let max_record_bytes: i64 = sqlx::query_scalar(
        "SELECT MAX(
               LENGTH(CAST(workspace_id AS BLOB))
                 + LENGTH(CAST(record_id AS BLOB))
                 + LENGTH(CAST(payload AS BLOB))
                 + 256
             )
             FROM e2ee_witness_records
             WHERE workspace_id = 'workspace-a'",
    )
    .fetch_one(target.pool())
    .await
    .unwrap();
    let max_repair_bytes = usize::try_from(max_record_bytes).unwrap() * 7;
    let mut cycles = 0;

    loop {
        let before: (i64, i64) = sqlx::query_as(
            "SELECT
                   COUNT(*),
                   COALESCE(SUM(
                     LENGTH(CAST(workspace_id AS BLOB))
                       + LENGTH(CAST(id AS BLOB))
                       + LENGTH(CAST(payload AS BLOB))
                       + 256
                   ), 0)
                 FROM e2ee_records",
        )
        .fetch_one(target.pool())
        .await
        .unwrap();
        let stats = apply_received_e2ee_replica_changes_with_witness_bounded(
            target.pool(),
            &workspace_keys,
            true,
            64,
            max_repair_bytes,
            &|| false,
        )
        .await
        .unwrap();
        let after: (i64, i64) = sqlx::query_as(
            "SELECT
                   COUNT(*),
                   COALESCE(SUM(
                     LENGTH(CAST(workspace_id AS BLOB))
                       + LENGTH(CAST(id AS BLOB))
                       + LENGTH(CAST(payload AS BLOB))
                       + 256
                   ), 0)
                 FROM e2ee_records",
        )
        .fetch_one(target.pool())
        .await
        .unwrap();
        let repaired_records = after.0 - before.0;
        let repaired_bytes = after.1 - before.1;
        assert!(repaired_records > 0);
        assert!(repaired_records <= 64);
        assert!(usize::try_from(repaired_bytes).unwrap() <= max_repair_bytes);
        cycles += 1;

        let pending = has_pending_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        assert_eq!(stats.remaining_replica_changes, pending);
        if !pending {
            break;
        }
        assert!(cycles < 130);
    }

    assert!(cycles > 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records")
            .fetch_one(target.pool())
            .await
            .unwrap(),
        i64::try_from(events.len()).unwrap()
    );
}

#[tokio::test]
async fn received_replica_rows_apply_in_bounded_cycles() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    for index in 0..20 {
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES (?, 'workspace-a', 'user-a', ?)",
        )
        .bind(format!("remote-session-{index:02}"))
        .bind(format!("Remote {index:02}"))
        .execute(source.pool())
        .await
        .unwrap();
    }
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();

    let target = test_db().await;
    copy_replica(source.pool(), target.pool()).await;
    let mut stats = apply_e2ee_replica_changes_inner(
        target.pool(),
        &workspace_keys,
        false,
        3,
        usize::MAX,
        &|| false,
    )
    .await
    .unwrap();
    assert!(stats.remaining_replica_changes);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sessions WHERE id LIKE 'remote-session-%'",
        )
        .fetch_one(target.pool())
        .await
        .unwrap(),
        3
    );

    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('local-session', 'workspace-a', 'user-a', 'Local')",
    )
    .execute(target.pool())
    .await
    .unwrap();
    let mut cycles = 1;
    while stats.remaining_replica_changes {
        stats = apply_e2ee_replica_changes_inner(
            target.pool(),
            &workspace_keys,
            false,
            3,
            usize::MAX,
            &|| false,
        )
        .await
        .unwrap();
        sqlx::query("UPDATE sessions SET title = title WHERE id = 'local-session'")
            .execute(target.pool())
            .await
            .unwrap();
        cycles += 1;
        assert!(cycles < 20);
    }

    assert!(cycles > 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sessions WHERE id LIKE 'remote-session-%'",
        )
        .fetch_one(target.pool())
        .await
        .unwrap(),
        20
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT title FROM sessions WHERE id = 'local-session'",)
            .fetch_one(target.pool())
            .await
            .unwrap(),
        "Local"
    );
}

#[tokio::test]
async fn cancelled_received_replica_preflight_is_bounded_and_releases_local_writes() {
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    let db = test_db().await;
    let mut transaction = db.pool().begin().await.unwrap();
    for index in 0..512 {
        let row_id = format!("remote-session-{index:04}");
        let sealed = key
            .seal_field(
                "workspace-a",
                "sessions",
                &row_id,
                ROW_MANIFEST_FIELD,
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                1,
                false,
                json!(true),
            )
            .unwrap();
        let payload_hash = anlg_e2ee::payload_hash(&sealed.payload);
        sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload)
                 VALUES (?, 'workspace-a', ?)",
        )
        .bind(&sealed.record_id)
        .bind(&sealed.payload)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_witness_records (
                   workspace_id, record_id, revision, writer_id, payload_hash, payload, sequence
                 ) VALUES (
                   'workspace-a', ?, 1, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', ?, ?, ?
                 )",
        )
        .bind(&sealed.record_id)
        .bind(payload_hash)
        .bind(&sealed.payload)
        .bind(i64::from(index) + 1)
        .execute(&mut *transaction)
        .await
        .unwrap();
    }
    transaction.commit().await.unwrap();

    let checks = AtomicUsize::new(0);
    let started_at = std::time::Instant::now();
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        apply_received_e2ee_replica_changes_with_witness_cancellable(
            db.pool(),
            &workspace_keys,
            false,
            || checks.fetch_add(1, AtomicOrdering::SeqCst) >= 20,
        ),
    )
    .await
    .expect("received-replica cancellation exceeded the activity deadline")
    .unwrap_err();

    assert!(matches!(error, E2eeReplicaError::Cancelled));
    assert!(started_at.elapsed() < std::time::Duration::from_secs(2));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sessions WHERE id LIKE 'remote-session-%'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('local-after-apply-cancel', 'workspace-a', 'user-a', 'Local')",
        )
        .execute(db.pool()),
    )
    .await
    .expect("cancelled received-replica preflight kept the database busy")
    .unwrap();
}

#[tokio::test]
async fn bounded_received_preflight_skips_foreign_and_unwitnessed_prefixes() {
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    let db = test_db().await;
    sqlx::query(
        "WITH RECURSIVE rows(id) AS (
               SELECT 1
               UNION ALL
               SELECT id + 1 FROM rows WHERE id < 256
             )
             INSERT INTO e2ee_records (id, workspace_id, payload)
             SELECT printf('!foreign-%04d', id), 'workspace-z', 'foreign' FROM rows",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "WITH RECURSIVE rows(id) AS (
               SELECT 1
               UNION ALL
               SELECT id + 1 FROM rows WHERE id < 256
             )
             INSERT INTO e2ee_records (id, workspace_id, payload)
             SELECT printf('!unwitnessed-%04d', id), 'workspace-a', 'unwitnessed' FROM rows",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let sealed = key
        .seal_field(
            "workspace-a",
            "sessions",
            "witnessed-session",
            ROW_MANIFEST_FIELD,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            1,
            false,
            json!(true),
        )
        .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_records (id, workspace_id, payload)
             VALUES (?, 'workspace-a', ?)",
    )
    .bind(&sealed.record_id)
    .bind(&sealed.payload)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_witness_records (
               workspace_id, record_id, revision, writer_id, payload_hash, payload, sequence
             ) VALUES (
               'workspace-a', ?, 1, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', ?, ?, 1
             )",
    )
    .bind(&sealed.record_id)
    .bind(anlg_e2ee::payload_hash(&sealed.payload))
    .bind(&sealed.payload)
    .execute(db.pool())
    .await
    .unwrap();

    let mut rejected_unwitnessed = 0;
    for _ in 0..6 {
        let stats =
            apply_received_e2ee_replica_changes_with_witness(db.pool(), &workspace_keys, false)
                .await
                .unwrap();
        assert!(
            stats.rejected_unwitnessed <= u64::try_from(E2EE_APPLY_PREFLIGHT_RECORD_LIMIT).unwrap()
        );
        rejected_unwitnessed += stats.rejected_unwitnessed;
        if sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sessions WHERE id = 'witnessed-session'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap()
            == 1
        {
            break;
        }
    }

    assert!(rejected_unwitnessed >= 256);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sessions WHERE id = 'witnessed-session'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn id_collision_does_not_record_unmaterialized_remote_state() {
    let recovery =
        RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc").unwrap();
    let workspace_keys = HashMap::from([
        (
            "workspace-a".to_string(),
            recovery.workspace_key("workspace-a").unwrap().into(),
        ),
        (
            "workspace-b".to_string(),
            recovery.workspace_key("workspace-b").unwrap().into(),
        ),
    ]);
    let source = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-b', 'user-b', 'Remote B')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();

    let target = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Local A')",
    )
    .execute(target.pool())
    .await
    .unwrap();
    copy_replica(source.pool(), target.pool()).await;
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    let remote_state_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_local_state
             WHERE workspace_id = 'workspace-b'
               AND table_name = 'sessions'
               AND row_id = 'session-1'",
    )
    .fetch_one(target.pool())
    .await
    .unwrap();
    let current_workspace: String =
        sqlx::query_scalar("SELECT workspace_id FROM sessions WHERE id = 'session-1'")
            .fetch_one(target.pool())
            .await
            .unwrap();
    assert_eq!(remote_state_count, 0);
    assert_eq!(current_workspace, "workspace-a");

    sqlx::query("DELETE FROM sessions WHERE id = 'session-1'")
        .execute(target.pool())
        .await
        .unwrap();
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    let remote: (String, String) =
        sqlx::query_as("SELECT workspace_id, title FROM sessions WHERE id = 'session-1'")
            .fetch_one(target.pool())
            .await
            .unwrap();
    assert_eq!(remote, ("workspace-b".to_string(), "Remote B".to_string()));
}

#[tokio::test]
async fn failed_remote_apply_rolls_back_the_guard() {
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
    copy_replica(source.pool(), target.pool()).await;
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET title = 'Remote' WHERE id = 'session-1'")
        .execute(source.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    let key = &workspace_keys["workspace-a"];
    let title_record_id = key.blind_field_id("sessions", "session-1", "title");
    let title_payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
        .bind(&title_record_id)
        .fetch_one(source.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(title_payload)
        .bind(title_record_id)
        .execute(target.pool())
        .await
        .unwrap();
    sqlx::query(
        "CREATE TRIGGER fail_remote_title_update
             BEFORE UPDATE OF title ON sessions
             BEGIN
               SELECT RAISE(ABORT, 'forced apply failure');
             END",
    )
    .execute(target.pool())
    .await
    .unwrap();

    assert!(
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .is_err()
    );
    let guard_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_apply_guard")
        .fetch_one(target.pool())
        .await
        .unwrap();
    assert_eq!(guard_count, 0);

    sqlx::query("DROP TRIGGER fail_remote_title_update")
        .execute(target.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET title = 'Local' WHERE id = 'session-1'")
        .execute(target.pool())
        .await
        .unwrap();
    let dirty_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
               AND table_name = 'sessions'
               AND row_id = 'session-1'",
    )
    .fetch_one(target.pool())
    .await
    .unwrap();
    assert_eq!(dirty_count, 1);
}

#[tokio::test]
async fn replica_apply_scan_bounds_payload_comparisons_before_filtering() {
    let workspace_keys = keys("workspace-a");
    let target = test_db().await;
    sqlx::query(
        "WITH RECURSIVE counter(value) AS (
           SELECT 0
           UNION ALL
           SELECT value + 1 FROM counter WHERE value < 255
         )
         INSERT INTO e2ee_witness_records (
           workspace_id, record_id, revision, writer_id,
           payload_hash, payload, sequence
         )
         SELECT
           'workspace-a',
           printf('record-%03d', value),
           1,
           'writer-a',
           printf('hash-%03d', value),
           printf('payload-%03d', value),
           value + 1
         FROM counter",
    )
    .execute(target.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_records (id, workspace_id, payload)
         SELECT record_id, workspace_id, payload FROM e2ee_witness_records",
    )
    .execute(target.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE e2ee_replica_payload_hashes
         SET payload_hash = (
           SELECT witness.payload_hash
           FROM e2ee_witness_records AS witness
           WHERE witness.record_id = e2ee_replica_payload_hashes.record_id
         )",
    )
    .execute(target.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_local_state (record_id, workspace_id, payload_hash, payload)
         SELECT record_id, workspace_id, payload_hash, payload FROM e2ee_witness_records",
    )
    .execute(target.pool())
    .await
    .unwrap();

    sqlx::query(
        "UPDATE e2ee_records
         SET payload = 'changed'
         WHERE id = 'record-255'",
    )
    .execute(target.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE e2ee_replica_payload_hashes
         SET payload_hash = 'changed-hash'
         WHERE record_id = 'record-255'",
    )
    .execute(target.pool())
    .await
    .unwrap();

    let changed = load_changed_e2ee_record_metadata(target.pool(), &workspace_keys)
        .await
        .unwrap();
    assert_eq!(
        changed.len(),
        usize::try_from(E2EE_APPLY_PREFLIGHT_RECORD_LIMIT).unwrap()
    );
    assert!(changed.iter().all(|record| !record.changed));

    sqlx::query("DELETE FROM e2ee_replica_pending WHERE record_id != 'record-255'")
        .execute(target.pool())
        .await
        .unwrap();
    let changed = load_changed_e2ee_record_metadata(target.pool(), &workspace_keys)
        .await
        .unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].id, "record-255");
    assert!(changed[0].changed);
}

use super::*;

#[tokio::test]
async fn encrypts_only_rows_marked_dirty() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    for id in ["session-1", "session-2"] {
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES (?, 'workspace-a', 'user-a', ?)",
        )
        .bind(id)
        .bind(id)
        .execute(db.pool())
        .await
        .unwrap();
    }
    sqlx::query(
        "DELETE FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
               AND table_name = 'sessions'
               AND row_id = 'session-2'",
    )
    .execute(db.pool())
    .await
    .unwrap();

    encrypt_e2ee_replica_changes_bounded(db.pool(), &workspace_keys, 64)
        .await
        .unwrap();
    let key = &workspace_keys["workspace-a"];
    let first_manifest = key.blind_field_id("sessions", "session-1", ROW_MANIFEST_FIELD);
    let second_manifest = key.blind_field_id("sessions", "session-2", ROW_MANIFEST_FIELD);
    let first_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM e2ee_records WHERE id = ?)")
            .bind(first_manifest)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let second_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM e2ee_records WHERE id = ?)")
            .bind(second_manifest)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(first_exists);
    assert!(!second_exists);
}

#[tokio::test]
async fn dirty_generation_preserves_a_concurrent_local_edit() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Before')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let writer_id = {
        let mut transaction = db.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
        let writer_id = load_or_create_writer_id(&mut transaction).await.unwrap();
        transaction.commit().await.unwrap();
        writer_id
    };
    let dirty = load_dirty_rows(db.pool(), &workspace_keys, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    let prepared = prepare_dirty_row(db.pool(), &workspace_keys["workspace-a"], &writer_id, dirty)
        .await
        .unwrap();

    sqlx::query("UPDATE sessions SET title = 'After' WHERE id = 'session-1'")
        .execute(db.pool())
        .await
        .unwrap();
    let persisted = persist_prepared_dirty_row(db.pool(), prepared)
        .await
        .unwrap();
    let generation: i64 = sqlx::query_scalar(
        "SELECT generation FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
               AND table_name = 'sessions'
               AND row_id = 'session-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let replica_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(persisted, 0);
    assert_eq!(generation, 2);
    assert_eq!(replica_count, 0);

    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();
    let key = &workspace_keys["workspace-a"];
    let title_record_id = key.blind_field_id("sessions", "session-1", "title");
    let payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
        .bind(&title_record_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let field = key
        .open_field("workspace-a", &title_record_id, &payload)
        .unwrap();
    assert_eq!(field.value, json!("After"));
}

#[tokio::test]
async fn capture_marker_inserted_after_prepare_prevents_transcript_persist() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    sqlx::query(
        "INSERT INTO transcripts (id, workspace_id, session_id, words_json)
             VALUES (
               'transcript-1',
               'workspace-a',
               'session-1',
               '[{\"text\":\"partial\"}]'
             )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let writer_id = {
        let mut transaction = db.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
        let writer_id = load_or_create_writer_id(&mut transaction).await.unwrap();
        transaction.commit().await.unwrap();
        writer_id
    };
    let dirty = load_dirty_rows(db.pool(), &workspace_keys, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    let prepared = prepare_dirty_row(db.pool(), &workspace_keys["workspace-a"], &writer_id, dirty)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO app_settings (id, value_json)
             VALUES ('capture_lifecycle_pending:session-1', ?)",
    )
    .bind(
        json!({
            "version": 1,
            "phase": "capturing",
            "sessionId": "session-1",
            "transcriptId": "transcript-1",
            "startedAt": 1_000,
            "createdAt": "2026-07-24T00:00:00.000Z",
            "audioOffsetMs": 0,
            "preserveExistingTranscript": false,
            "ownerUserId": "workspace-a",
            "memo": ""
        })
        .to_string(),
    )
    .execute(db.pool())
    .await
    .unwrap();

    let persisted = persist_prepared_dirty_row_inner(db.pool(), prepared, true)
        .await
        .unwrap();
    let dirty_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_dirty_rows
             WHERE table_name = 'transcripts' AND row_id = 'transcript-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let local_state_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_local_state
             WHERE table_name = 'transcripts' AND row_id = 'transcript-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(persisted, 0);
    assert_eq!(dirty_count, 1);
    assert_eq!(local_state_count, 0);

    sqlx::query(
        "UPDATE app_settings
             SET value_json = json_set(value_json, '$.phase', 'finalizing')
             WHERE id = 'capture_lifecycle_pending:session-1'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes_deferring_active_captures(db.pool(), &workspace_keys)
        .await
        .unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn cancelled_snapshot_finishes_current_row_and_releases_local_writes() {
    let calibration_db = test_db().await;
    let calibration_keys = keys("workspace-a");
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('calibration', 'workspace-a', 'user-a', 'Snapshot row')",
    )
    .execute(calibration_db.pool())
    .await
    .unwrap();
    let calibration_checks = std::sync::atomic::AtomicUsize::new(0);
    encrypt_e2ee_replica_changes_bounded_deferring_active_captures_cancellable(
        calibration_db.pool(),
        &calibration_keys,
        E2EE_ENCRYPT_ROW_LIMIT,
        || {
            calibration_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            false
        },
    )
    .await
    .unwrap();
    let cancel_after_first_persist = calibration_checks
        .load(std::sync::atomic::Ordering::SeqCst)
        .checked_sub(3)
        .unwrap();

    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    for index in 0..129 {
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES (?, 'workspace-a', 'user-a', 'Snapshot row')",
        )
        .bind(format!("session-{index:03}"))
        .execute(db.pool())
        .await
        .unwrap();
    }
    let initial_dirty_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(initial_dirty_rows, 129);

    let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
    let stats = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        encrypt_e2ee_replica_changes_bounded_deferring_active_captures_cancellable(
            db.pool(),
            &workspace_keys,
            E2EE_ENCRYPT_ROW_LIMIT,
            || {
                cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    >= cancel_after_first_persist
            },
        ),
    )
    .await
    .expect("snapshot cancellation exceeded the activity deadline")
    .unwrap();

    assert!(stats.remaining_replica_changes);
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(remaining, 128);

    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-cancel', 'workspace-a', 'user-a', 'Local write')",
        )
        .execute(db.pool()),
    )
    .await
    .expect("cancelled snapshot kept the database busy")
    .unwrap();
}

#[tokio::test]
async fn large_dirty_queue_cancellation_stays_bounded_and_releases_local_writes() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    sqlx::query(
        "WITH RECURSIVE rows(id) AS (
                SELECT 1
                UNION ALL
                SELECT id + 1 FROM rows WHERE id < 100000
            )
            INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
            SELECT 'workspace-a', 'sessions', printf('missing-%06d', id) FROM rows",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let (page, remaining) =
        load_dirty_rows_page(db.pool(), &workspace_keys, E2EE_ENCRYPT_ROW_LIMIT, true)
            .await
            .unwrap();
    assert_eq!(page.len(), E2EE_ENCRYPT_ROW_LIMIT as usize);
    assert!(remaining);

    let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
    let stats = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        encrypt_e2ee_replica_changes_deferring_active_captures_cancellable(
            db.pool(),
            &workspace_keys,
            || cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= 1,
        ),
    )
    .await
    .expect("large dirty queue cancellation exceeded the activity deadline")
    .unwrap();

    assert!(stats.remaining_replica_changes);
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(remaining, 100_000);
    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-large-cancel', 'workspace-a', 'user-a', 'Local write')",
        )
        .execute(db.pool()),
    )
    .await
    .expect("large dirty queue cancellation kept the database busy")
    .unwrap();
}

#[tokio::test]
async fn witness_change_preserves_a_prepared_dirty_row() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Local')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let writer_id = {
        let mut transaction = db.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
        let writer_id = load_or_create_writer_id(&mut transaction).await.unwrap();
        transaction.commit().await.unwrap();
        writer_id
    };
    let dirty = load_dirty_rows(db.pool(), &workspace_keys, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    let prepared = prepare_dirty_row(db.pool(), key, &writer_id, dirty)
        .await
        .unwrap();

    let witnessed = key
        .seal_field(
            "workspace-a",
            "sessions",
            "session-1",
            "title",
            "ffffffffffffffffffffffffffffffff",
            5,
            false,
            json!("Remote"),
        )
        .unwrap();
    merge_e2ee_witness_events(
        db.pool(),
        key,
        "workspace-a",
        &[E2eeWitnessEvent {
            sequence: 1,
            record_id: witnessed.record_id,
            workspace_id: "workspace-a".to_string(),
            payload_hash: anlg_e2ee::payload_hash(&witnessed.payload),
            payload: witnessed.payload,
        }],
    )
    .await
    .unwrap();

    assert_eq!(
        persist_prepared_dirty_row(db.pool(), prepared)
            .await
            .unwrap(),
        0
    );
    let dirty_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let replica_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(dirty_count, 1);
    assert_eq!(replica_count, 0);

    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();
    let title_id = key.blind_field_id("sessions", "session-1", "title");
    let payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
        .bind(&title_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let field = key.open_field("workspace-a", &title_id, &payload).unwrap();
    assert_eq!(field.revision, 6);
    assert_eq!(field.value, json!("Local"));
}

#[tokio::test]
async fn bounded_encryption_makes_deterministic_progress() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    for index in 0..65 {
        let id = format!("session-{index:03}");
        sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES (?, 'workspace-a')")
            .bind(id)
            .execute(db.pool())
            .await
            .unwrap();
    }

    encrypt_e2ee_replica_changes_bounded(db.pool(), &workspace_keys, 1_000)
        .await
        .unwrap();
    let remaining: Vec<String> = sqlx::query_scalar(
        "SELECT row_id FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
             ORDER BY row_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(remaining, ["session-064"]);
    assert!(
        has_pending_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap()
    );

    encrypt_e2ee_replica_changes_bounded(db.pool(), &workspace_keys, 64)
        .await
        .unwrap();
    let remaining_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(remaining_count, 0);
    assert!(
        !has_pending_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn active_capture_transcripts_do_not_starve_unrelated_rows() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    for index in 0..65 {
        let transcript_id = format!("protected-{index:03}");
        let session_id = format!("session-{index:03}");
        sqlx::query(
            "INSERT INTO transcripts (id, workspace_id, words_json)
                 VALUES (?, 'workspace-a', '[{\"text\":\"partial\"}]')",
        )
        .bind(&transcript_id)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
            .bind(format!("capture_lifecycle_pending:{session_id}"))
            .bind(
                json!({
                    "version": 1,
                    "phase": "capturing",
                    "sessionId": session_id,
                    "transcriptId": transcript_id,
                    "startedAt": 1_000,
                    "createdAt": "2026-07-24T00:00:00.000Z",
                    "audioOffsetMs": 0,
                    "preserveExistingTranscript": false,
                    "ownerUserId": "workspace-a",
                    "memo": ""
                })
                .to_string(),
            )
            .execute(db.pool())
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO transcripts (id, workspace_id, words_json)
             VALUES ('unrelated', 'workspace-a', '[{\"text\":\"complete\"}]')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    encrypt_e2ee_replica_changes_bounded_deferring_active_captures(db.pool(), &workspace_keys, 64)
        .await
        .unwrap();

    let remaining_protected: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_dirty_rows
             WHERE table_name = 'transcripts' AND row_id GLOB 'protected-*'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let unrelated_dirty: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_dirty_rows
             WHERE table_name = 'transcripts' AND row_id = 'unrelated'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let unrelated_records: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_local_state
             WHERE table_name = 'transcripts' AND row_id = 'unrelated'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(remaining_protected, 65);
    assert_eq!(unrelated_dirty, 0);
    assert!(unrelated_records > 0);
    assert!(
        !has_pending_e2ee_replica_changes_deferring_active_captures(db.pool(), &workspace_keys,)
            .await
            .unwrap()
    );
    assert!(
        has_pending_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap()
    );

    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(remaining, 0);
}

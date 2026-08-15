use super::*;
use crate::runtime::e2ee_sync::ReplicaSyncOutcome;

#[tokio::test]
async fn replica_transport_syncs_without_the_cloudsync_extension() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let hook = E2eeSyncHook::default();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    hook.set_personal_workspace("workspace-1", &recovery_key)
        .unwrap();
    let witness_state = InitiallyUninitializedWitness::default();
    witness_state.initialized.store(true, Ordering::SeqCst);
    let witness_server = MockServer::start().await;
    Mock::given(path("/sync/e2ee/witness/workspace-1"))
        .respond_with(witness_state)
        .mount(&witness_server)
        .await;
    hook.set_replica_witness(
        crate::e2ee_witness::E2eeWitnessClient::new(
            crate::CloudsyncE2eeWitness {
                endpoint: format!("{}/sync/e2ee/witness/workspace-1", witness_server.uri()),
                access_token: "access-token".to_string(),
            },
            "workspace-1",
        )
        .unwrap(),
    );
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, title)
         VALUES ('session-1', 'workspace-1', 'Portable sync')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    assert_eq!(
        hook.sync_replica_transport(db.pool()).await.unwrap(),
        ReplicaSyncOutcome::Settled
    );
    assert!(
        witness_server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|request| request.method == wiremock::http::Method::POST)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_dirty_rows")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    assert!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_witness_records")
            .fetch_one(db.pool())
            .await
            .unwrap()
            > 0
    );
}

#[test]
fn paused_replica_sync_preserves_the_last_completed_result() {
    let hook = E2eeSyncHook::default();
    hook.replica_sync_succeeded();
    hook.replica_sync_failed(&std::io::Error::other("temporary failure"));
    let before = hook.replica_status();

    hook.replica_sync_started();
    hook.replica_sync_paused();

    let after = hook.replica_status();
    assert!(!after.syncing);
    assert_eq!(after.last_sync_at_ms, before.last_sync_at_ms);
    assert_eq!(after.last_error, before.last_error);
    assert_eq!(after.consecutive_failures, before.consecutive_failures);
}

#[tokio::test]
async fn replica_transport_hydrates_a_fresh_local_database() {
    let source = Db::connect_memory_plain().await.unwrap();
    let target = Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&source).await.unwrap();
    anlg_db_app::prepare_schema(&target).await.unwrap();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let (witness_server, witness_config) =
        crate::tests::support::setup_witness("workspace-1").await;
    let configure = |hook: &E2eeSyncHook| {
        hook.set_personal_workspace("workspace-1", &recovery_key)
            .unwrap();
        hook.set_replica_witness(
            crate::e2ee_witness::E2eeWitnessClient::new(witness_config.clone(), "workspace-1")
                .unwrap(),
        );
    };
    let source_hook = E2eeSyncHook::default();
    let target_hook = E2eeSyncHook::default();
    configure(&source_hook);
    configure(&target_hook);
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, title)
         VALUES ('session-1', 'workspace-1', 'Portable hydration')",
    )
    .execute(source.pool())
    .await
    .unwrap();

    source_hook
        .sync_replica_transport(source.pool())
        .await
        .unwrap();
    target_hook
        .sync_replica_transport(target.pool())
        .await
        .unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(target.pool())
            .await
            .unwrap(),
        "Portable hydration"
    );
    assert!(!witness_server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn capture_lifecycle_marker_defers_only_its_transcript() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let hook = E2eeSyncHook::default();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    hook.set_personal_workspace("workspace-1", &recovery_key)
        .unwrap();
    let witness_state = InitiallyUninitializedWitness::default();
    witness_state.initialized.store(true, Ordering::SeqCst);
    let witness_server = MockServer::start().await;
    Mock::given(path("/sync/e2ee/witness/workspace-1"))
        .respond_with(witness_state)
        .mount(&witness_server)
        .await;
    hook.set_witness(
        crate::e2ee_witness::E2eeWitnessClient::new(
            crate::CloudsyncE2eeWitness {
                endpoint: format!("{}/sync/e2ee/witness/workspace-1", witness_server.uri()),
                access_token: "access-token".to_string(),
            },
            "workspace-1",
        )
        .unwrap(),
    );

    sqlx::query(
        "INSERT INTO app_settings (id, value_json)
             VALUES ('capture_lifecycle_pending:session-1', ?)",
    )
    .bind(
        serde_json::json!({
            "version": 1,
            "phase": "capturing",
            "sessionId": "session-1",
            "transcriptId": "transcript-1",
            "startedAt": 1_000,
            "createdAt": "2026-07-24T00:00:00.000Z",
            "audioOffsetMs": 0,
            "preserveExistingTranscript": false,
            "ownerUserId": "workspace-1",
            "memo": ""
        })
        .to_string(),
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, title)
             VALUES
               ('session-1', 'workspace-1', 'Active capture'),
               ('session-2', 'workspace-1', 'Unrelated edit')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO transcripts (id, workspace_id, session_id, words_json)
             VALUES
               ('transcript-1', 'workspace-1', 'session-1', '[{\"text\":\"partial\"}]'),
               ('transcript-2', 'workspace-1', 'session-2', '[{\"text\":\"complete\"}]')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    assert_eq!(
        anlg_db_core::CloudsyncSyncHook::before_sync(&hook, db.pool())
            .await
            .unwrap(),
        anlg_db_core::CloudsyncSyncDirective::SendAndReceive
    );
    assert!(!witness_server.received_requests().await.unwrap().is_empty());
    let remaining_dirty: Vec<(String, String)> = sqlx::query_as(
        "SELECT table_name, row_id
             FROM e2ee_dirty_rows
             ORDER BY table_name, row_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        remaining_dirty,
        vec![("transcripts".to_string(), "transcript-1".to_string())]
    );
    let protected_records: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM e2ee_local_state
             WHERE table_name = 'transcripts' AND row_id = 'transcript-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(protected_records, 0);
    let unrelated_records: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM e2ee_local_state
             WHERE table_name = 'transcripts' AND row_id = 'transcript-2'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(unrelated_records > 0);

    let result: anlg_db_core::CloudsyncNetworkResult = serde_json::from_value(serde_json::json!({
        "receive": {
            "rows": 0,
            "tables": [],
            "chunks": 0,
            "bytes": 0,
            "complete": true
        }
    }))
    .unwrap();
    let outcome = anlg_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &result)
        .await
        .unwrap();
    assert!(!outcome.local_work_remaining);
    let keys = hook.snapshot();

    sqlx::query(
        "UPDATE app_settings
             SET value_json = json_set(value_json, '$.phase', 'finalizing')
             WHERE id = 'capture_lifecycle_pending:session-1'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        anlg_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(db.pool(), &keys,)
            .await
            .unwrap()
    );

    sqlx::query(
        "INSERT INTO app_settings (id, value_json)
             VALUES
               ('capture_lifecycle_pending:malformed', '{}'),
               ('capture_lifecycle_pending:invalid-json', '{')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        anlg_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(db.pool(), &keys,)
            .await
            .unwrap()
    );

    sqlx::query(
        "UPDATE app_settings
             SET value_json = json_remove(value_json, '$.phase')
             WHERE id = 'capture_lifecycle_pending:session-1'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        !anlg_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(db.pool(), &keys,)
            .await
            .unwrap()
    );

    sqlx::query(
        "UPDATE app_settings
             SET value_json = json_set(value_json, '$.phase', 'unknown')
             WHERE id = 'capture_lifecycle_pending:session-1'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        !anlg_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(db.pool(), &keys,)
            .await
            .unwrap()
    );

    sqlx::query(
        "UPDATE app_settings
             SET value_json = json_set(value_json, '$.summaryMode', 'if_empty')
             WHERE id = 'capture_lifecycle_pending:session-1'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        anlg_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(db.pool(), &keys,)
            .await
            .unwrap()
    );

    sqlx::query(
        "UPDATE app_settings
             SET value_json = json_remove(
               json_set(value_json, '$.version', json('true')),
               '$.summaryMode'
             )
             WHERE id = 'capture_lifecycle_pending:session-1'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        anlg_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(db.pool(), &keys,)
            .await
            .unwrap()
    );

    sqlx::query(
        "UPDATE app_settings
             SET value_json = json_set(
               value_json,
               '$.version',
               1,
               '$.startedAt',
               1e999
             )
             WHERE id = 'capture_lifecycle_pending:session-1'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        anlg_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(db.pool(), &keys,)
            .await
            .unwrap()
    );

    sqlx::query(
        "UPDATE app_settings
             SET value_json = json_set(
               value_json,
               '$.startedAt',
               1000,
               '$.summaryMode',
               'if_empty'
             )
             WHERE id = 'capture_lifecycle_pending:session-1'",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let outcome = anlg_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &result)
        .await
        .unwrap();
    assert!(outcome.local_work_remaining);

    assert_eq!(
        anlg_db_core::CloudsyncSyncHook::before_sync(&hook, db.pool())
            .await
            .unwrap(),
        anlg_db_core::CloudsyncSyncDirective::SendAndReceive
    );
    let protected_dirty: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM e2ee_dirty_rows
             WHERE table_name = 'transcripts' AND row_id = 'transcript-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(protected_dirty, 0);
}

#[tokio::test]
async fn idle_sync_skips_replica_reconciliation_until_new_work_arrives() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let (hook, _witness_server) = configured_test_hook().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-1', 'workspace-1', 'Local session')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    anlg_db_core::CloudsyncSyncHook::before_sync(&hook, db.pool())
        .await
        .unwrap();
    let complete_idle = receive_result(0, true);
    anlg_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &complete_idle)
        .await
        .unwrap();

    let (record_id, original_payload): (String, String) = sqlx::query_as(
        "SELECT replica.id, replica.payload
             FROM e2ee_records AS replica
             INNER JOIN e2ee_witness_records AS witness
              ON witness.workspace_id = replica.workspace_id
             AND witness.record_id = replica.id
              AND witness.payload_hash = replica.payload_hash
             WHERE replica.workspace_id = 'workspace-1'
             ORDER BY replica.id
             LIMIT 1",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE e2ee_records SET payload = 'corrupt' WHERE id = ?")
        .bind(&record_id)
        .execute(db.pool())
        .await
        .unwrap();

    let idle_outcome =
        anlg_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &complete_idle)
            .await
            .unwrap();
    let idle_payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
        .bind(&record_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(idle_payload, "corrupt");
    assert!(!idle_outcome.local_work_remaining);

    let delivered = receive_result(1, true);
    anlg_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &delivered)
        .await
        .unwrap();
    let reconciled_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(record_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(reconciled_payload, original_payload);
}

#[tokio::test]
async fn nonfinal_receive_keeps_reconciliation_pending_for_final_snapshot() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let (hook, _witness_server) = configured_test_hook().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-1', 'workspace-1', 'Remote session')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    anlg_db_core::CloudsyncSyncHook::before_sync(&hook, db.pool())
        .await
        .unwrap();
    anlg_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &receive_result(0, true))
        .await
        .unwrap();
    let missing_record_id: String = sqlx::query_scalar(
        "SELECT replica.id
             FROM e2ee_records AS replica
             INNER JOIN e2ee_witness_records AS witness
               ON witness.workspace_id = replica.workspace_id
              AND witness.record_id = replica.id
             WHERE replica.workspace_id = 'workspace-1'
             ORDER BY replica.id
             LIMIT 1",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("DELETE FROM e2ee_records WHERE id = ?")
        .bind(&missing_record_id)
        .execute(db.pool())
        .await
        .unwrap();

    let partial_outcome =
        anlg_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &receive_result(1, false))
            .await
            .unwrap();
    assert!(partial_outcome.local_work_remaining);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records WHERE id = ?")
            .bind(&missing_record_id)
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );

    let final_outcome =
        anlg_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &receive_result(0, true))
            .await
            .unwrap();
    assert!(!final_outcome.local_work_remaining);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records WHERE id = ?")
            .bind(&missing_record_id)
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );

    sqlx::query("DELETE FROM e2ee_records WHERE id = ?")
        .bind(&missing_record_id)
        .execute(db.pool())
        .await
        .unwrap();
    let failed_after_chunks: anlg_db_core::CloudsyncNetworkResult =
        serde_json::from_value(serde_json::json!({
            "receive": {
                "rows": 1,
                "tables": ["e2ee_records"],
                "chunks": 1,
                "bytes": 1,
                "complete": false,
                "error": "later chunk failed"
            }
        }))
        .unwrap();
    let failed_outcome =
        anlg_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &failed_after_chunks)
            .await
            .unwrap();
    assert!(failed_outcome.local_work_remaining);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records WHERE id = ?")
            .bind(&missing_record_id)
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );

    let retry_outcome =
        anlg_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &receive_result(0, true))
            .await
            .unwrap();
    assert!(!retry_outcome.local_work_remaining);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records WHERE id = ?")
            .bind(missing_record_id)
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn activity_drains_large_no_mismatch_preflight_before_local_write() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = std::sync::Arc::new(PluginDbRuntime::new(std::sync::Arc::clone(&db)));
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    runtime
        .set_e2ee_recovery_key("workspace-1", &recovery_key)
        .unwrap();
    let (configured_hook, _witness_server) = configured_test_hook().await;
    runtime
        .e2ee_sync_hook
        .set_witness(configured_hook.witness().unwrap());

    sqlx::query(
        "WITH RECURSIVE rows(id) AS (
               SELECT 1
               UNION ALL
               SELECT id + 1 FROM rows WHERE id < 50000
             )
             INSERT INTO e2ee_records (id, workspace_id, payload)
             SELECT printf('!unwitnessed-%06d', id), 'workspace-1', 'unwitnessed' FROM rows",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_local_state (
               record_id, workspace_id, table_name, row_id, field_name, revision,
               writer_id, value_tag, payload_hash, payload
             )
             SELECT
               id,
               workspace_id,
               'sessions',
               id,
               '$row',
               1,
               'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
               'unchanged',
               'unchanged',
               payload
             FROM e2ee_records
             WHERE id LIKE '!unwitnessed-%'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let key = runtime.workspace_key("workspace-1").unwrap();
    let mut transaction = db.pool().begin().await.unwrap();
    for index in 0..32 {
        let sealed = key
            .seal_field(
                "workspace-1",
                "sessions",
                &format!("received-session-{index:02}"),
                "$row",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                1,
                false,
                serde_json::json!(true),
            )
            .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload)
                 VALUES (?, 'workspace-1', ?)",
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
                   'workspace-1', ?, 1, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', ?, ?, ?
                 )",
        )
        .bind(&sealed.record_id)
        .bind(anlg_e2ee::payload_hash(&sealed.payload))
        .bind(&sealed.payload)
        .bind(i64::from(index) + 1)
        .execute(&mut *transaction)
        .await
        .unwrap();
    }
    transaction.commit().await.unwrap();

    let reconcile_runtime = std::sync::Arc::clone(&runtime);
    let reconcile_db = std::sync::Arc::clone(&db);
    let reconcile = tokio::spawn(async move {
        anlg_db_core::CloudsyncSyncHook::after_sync(
            reconcile_runtime.e2ee_sync_hook.as_ref(),
            reconcile_db.pool(),
            &receive_result(1, false),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while runtime
            .e2ee_sync_hook
            .received_apply_cancellation_checks
            .load(Ordering::Acquire)
            < 70
        {
            assert!(
                !reconcile.is_finished(),
                "received-replica reconciliation finished before cancellation"
            );
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("received-replica reconciliation did not enter bounded preflight");

    let cancelled_at = std::time::Instant::now();
    runtime
        .e2ee_sync_hook
        .begin_activity("capture".to_string(), "session-1".to_string());
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(2), reconcile)
        .await
        .expect("activity did not drain received-replica reconciliation")
        .unwrap()
        .unwrap();
    assert!(outcome.deferred);
    assert!(outcome.local_work_remaining);
    assert!(cancelled_at.elapsed() < std::time::Duration::from_secs(2));

    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('local-after-received-cancel', 'workspace-1', 'workspace-1', 'Local')",
        )
        .execute(db.pool()),
    )
    .await
    .expect("local write remained blocked after received-replica cancellation")
    .unwrap();
    assert!(runtime.e2ee_sync_hook.end_activity("capture", "session-1"));
    runtime.e2ee_sync_hook.notify_activity_changed();
}

#[test]
fn reconciliation_epochs_preserve_newer_requests() {
    let hook = E2eeSyncHook::default();
    hook.request_reconciliation();
    let first_epoch = hook.reconciliation_request_epoch().unwrap();

    hook.request_reconciliation();
    hook.complete_reconciliation(first_epoch);

    assert!(hook.reconciliation_requested());
    let second_epoch = hook.reconciliation_request_epoch().unwrap();
    assert!(second_epoch > first_epoch);
    hook.complete_reconciliation(second_epoch);
    assert!(!hook.reconciliation_requested());

    hook.request_reconciliation();
    hook.clear();
    assert!(!hook.reconciliation_requested());
    hook.request_reconciliation();
    assert!(hook.reconciliation_requested());
}

#[tokio::test]
async fn witness_refresh_failure_keeps_reconciliation_pending() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let hook = E2eeSyncHook::default();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    hook.set_personal_workspace("workspace-1", &recovery_key)
        .unwrap();
    let witness_server = MockServer::start().await;
    Mock::given(path("/sync/e2ee/witness/workspace-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "initialized": true,
            "initializedAt": "2026-07-24T00:00:00Z",
            "headSequence": 1,
            "throughSequence": 1,
            "nextAfterSequence": 1,
            "events": [{
                "sequence": 1,
                "recordId": "invalid-record",
                "payloadHash": "invalid-hash",
                "payload": "invalid-payload"
            }]
        })))
        .mount(&witness_server)
        .await;
    hook.set_witness(
        crate::e2ee_witness::E2eeWitnessClient::new(
            crate::CloudsyncE2eeWitness {
                endpoint: format!("{}/sync/e2ee/witness/workspace-1", witness_server.uri()),
                access_token: "access-token".to_string(),
            },
            "workspace-1",
        )
        .unwrap(),
    );
    let setup_epoch = hook.reconciliation_request_epoch().unwrap();
    hook.complete_reconciliation(setup_epoch);

    assert!(
        anlg_db_core::CloudsyncSyncHook::before_sync(&hook, db.pool())
            .await
            .is_err()
    );
    assert!(hook.reconciliation_requested());
}

#[tokio::test]
async fn failed_reconciliation_epoch_stays_pending() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let (hook, _witness_server) = configured_test_hook().await;
    let setup_epoch = hook.reconciliation_request_epoch().unwrap();
    hook.complete_reconciliation(setup_epoch);
    sqlx::query(
        "INSERT INTO e2ee_witness_records (
               workspace_id,
               record_id,
               revision,
               writer_id,
               payload_hash,
               payload,
               sequence
             )
             VALUES (
               'workspace-1',
               'invalid-record',
               1,
               'invalid-writer',
               'invalid-hash',
               'invalid-payload',
               1
             )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    assert!(
        anlg_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &receive_result(1, true),)
            .await
            .is_err()
    );
    assert!(hook.reconciliation_requested());
}

#[tokio::test]
async fn shared_witness_rejection_blocks_the_cloudsync_network_phase() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let hook = E2eeSyncHook::default();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let shared_key = anlg_e2ee::WorkspaceKey::generate().unwrap();
    hook.set_workspaces(
        "personal",
        &recovery_key,
        HashMap::from([(
            "shared".to_string(),
            anlg_e2ee::WorkspaceKeyring::new(shared_key),
        )]),
    )
    .unwrap();

    let server = MockServer::start().await;
    let initialized_page = || {
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "initialized": true,
            "initializedAt": "2026-08-15T00:00:00Z",
            "headSequence": 0,
            "throughSequence": 0,
            "nextAfterSequence": 0,
            "events": [],
        }))
    };
    Mock::given(wiremock::matchers::method("GET"))
        .and(path("/sync/e2ee/witness/personal"))
        .respond_with(initialized_page())
        .mount(&server)
        .await;
    Mock::given(wiremock::matchers::method("POST"))
        .and(path("/sync/e2ee/witness/personal"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "initializedAt": "2026-08-15T00:00:00Z",
            "headSequence": 0,
        })))
        .mount(&server)
        .await;
    Mock::given(wiremock::matchers::method("GET"))
        .and(path("/sync/e2ee/witness/shared"))
        .respond_with(initialized_page())
        .mount(&server)
        .await;
    Mock::given(wiremock::matchers::method("POST"))
        .and(path("/sync/e2ee/witness/shared"))
        .respond_with(ResponseTemplate::new(403))
        .expect(1)
        .mount(&server)
        .await;
    let personal_witness = crate::e2ee_witness::E2eeWitnessClient::new(
        crate::CloudsyncE2eeWitness {
            endpoint: format!("{}/sync/e2ee/witness/personal", server.uri()),
            access_token: "access-token".to_string(),
        },
        "personal",
    )
    .unwrap();
    let shared_witness = personal_witness.for_workspace("shared").unwrap();
    hook.set_witnesses(HashMap::from([
        ("personal".to_string(), personal_witness),
        ("shared".to_string(), shared_witness),
    ]));
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('shared-session', 'shared', 'member', 'Retired key write')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let error = anlg_db_core::CloudsyncSyncHook::before_sync(&hook, db.pool())
        .await
        .unwrap_err();

    assert!(error.to_string().contains("403 Forbidden"));
}

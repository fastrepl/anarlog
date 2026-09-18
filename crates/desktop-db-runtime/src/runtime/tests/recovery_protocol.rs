use super::*;

#[tokio::test]
async fn poisoned_replica_recovery_requires_the_disposable_e2ee_table_only() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    anlg_db_app::claim_cloudsync_workspace(db.pool(), "workspace-1")
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE cloudsync_table_settings (
               tbl_name TEXT NOT NULL,
               col_name TEXT NOT NULL,
               key TEXT NOT NULL,
               value TEXT,
               PRIMARY KEY (tbl_name, col_name, key)
             )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO cloudsync_table_settings (tbl_name, col_name, key, value)
             VALUES ('e2ee_records', '*', 'filter', 'workspace_id')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let runtime = DesktopDbRuntime::<TestQueryEventSink>::for_test(std::sync::Arc::clone(&db));

    require_disposable_cloudsync_replica(runtime.db.as_ref(), CloudsyncOperationCancellation::None)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO cloudsync_table_settings (tbl_name, col_name, key, value)
             VALUES ('sessions', '*', 'filter', 'workspace_id')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        require_disposable_cloudsync_replica(
            runtime.db.as_ref(),
            CloudsyncOperationCancellation::None,
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("sessions")
    );
    let key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap()
    .workspace_key("workspace-1")
    .unwrap();
    assert!(
        prepare_cloudsync_poison_recovery(
            runtime.db.as_ref(),
            "workspace-1",
            "workspace-1",
            &key,
            CloudsyncOperationCancellation::None,
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("sessions")
    );
    assert_eq!(
        anlg_db_app::cloudsync_full_resync_generation(runtime.db.pool())
            .await
            .unwrap(),
        None
    );
    assert!(
        anlg_db_app::cloudsync_recovery_state(runtime.db.pool())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn native_replica_logout_preserves_domain_rows_and_reinstalls_an_empty_filter() {
    let db = Db::connect_memory().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    anlg_db_app::set_cloudsync_personal_write_scope(db.pool(), "workspace-1")
        .await
        .unwrap();
    db.cloudsync_init(CLOUDSYNC_REPLICA_TABLE, None, None)
        .await
        .unwrap();
    db.cloudsync_set_filter(CLOUDSYNC_REPLICA_TABLE, CLOUDSYNC_WRITE_FILTER)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, title)
             VALUES ('session-1', 'workspace-1', 'Preserved')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_records (id, workspace_id, payload)
             VALUES ('record-1', 'workspace-1', 'ciphertext')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_local_state (
               record_id, workspace_id, table_name, row_id, field_name,
               revision, value_tag, payload_hash, writer_id, payload
             ) VALUES (
               'record-1', 'workspace-1', 'sessions', 'session-1', 'title',
               1, 'value-tag', 'payload-hash', 'writer-1', 'ciphertext'
             )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let local_changes_before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records_cloudsync")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(local_changes_before > 0);
    let dirty_rows_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(dirty_rows_before > 0);
    let local_state_before: (String, String) = sqlx::query_as(
        "SELECT payload_hash, payload
             FROM e2ee_local_state
             WHERE record_id = 'record-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();

    db.cloudsync_network_logout().await.unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sessions")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    db.cloudsync_init(CLOUDSYNC_REPLICA_TABLE, None, None)
        .await
        .unwrap();
    db.cloudsync_set_filter(CLOUDSYNC_REPLICA_TABLE, CLOUDSYNC_WRITE_FILTER)
        .await
        .unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records_cloudsync")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
                 FROM cloudsync_table_settings
                 WHERE tbl_name = 'e2ee_records'
                   AND col_name = '*'
                   AND key = 'filter'
                   AND value = ?",
        )
        .bind(CLOUDSYNC_WRITE_FILTER)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_dirty_rows")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        dirty_rows_before
    );
    assert_eq!(
        sqlx::query_as::<_, (String, String)>(
            "SELECT payload_hash, payload
                 FROM e2ee_local_state
                 WHERE record_id = 'record-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        local_state_before
    );
}

#[tokio::test]
async fn legacy_cutover_snapshots_local_state_before_initializing_the_witness() {
    let db = std::sync::Arc::new(Db::connect_memory().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    db.cloudsync_init("sessions", None, None).await.unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-1', 'workspace-1', 'Legacy session')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_local_state")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );

    let runtime = DesktopDbRuntime::<TestQueryEventSink>::for_test(std::sync::Arc::clone(&db));
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    runtime
        .set_e2ee_recovery_key("workspace-1", &recovery_key)
        .unwrap();
    let workspace_key = runtime.workspace_key("workspace-1").unwrap();
    let witness_state = InitiallyUninitializedWitness::default();
    let witness_server = MockServer::start().await;
    Mock::given(path("/sync/e2ee/witness/workspace-1"))
        .respond_with(witness_state.clone())
        .mount(&witness_server)
        .await;
    let witness = crate::e2ee_witness::E2eeWitnessClient::new(
        crate::CloudsyncE2eeWitness {
            endpoint: format!("{}/sync/e2ee/witness/workspace-1", witness_server.uri()),
            access_token: "access-token".to_string(),
        },
        "workspace-1",
    )
    .unwrap();

    let cancellation = crate::e2ee_witness::E2eeWitnessCancellation::default();
    runtime
        .prepare_e2ee_cutover_and_initialize_witnesses(
            &HashMap::from([("workspace-1".to_string(), witness)]),
            &HashMap::from([(
                "workspace-1".to_string(),
                anlg_e2ee::WorkspaceKeyring::new(workspace_key),
            )]),
            &cancellation,
        )
        .await
        .unwrap();

    assert!(witness_state.initialized.load(Ordering::SeqCst));
    assert!(
        anlg_db_app::has_e2ee_local_state(db.pool(), "workspace-1")
            .await
            .unwrap()
    );
    assert!(
        !anlg_db_core::cloudsync_is_enabled_on(db.pool(), "sessions")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn witness_hydration_drains_more_than_one_replica_apply_batch() {
    check_witness_hydration_drains(false).await;
}

#[tokio::test]
async fn witness_hydration_drains_applicable_rows_around_incomplete_transcripts() {
    check_witness_hydration_drains(true).await;
}

async fn check_witness_hydration_drains(incomplete_transcript: bool) {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = DesktopDbRuntime::<TestQueryEventSink>::for_test(std::sync::Arc::clone(&db));
    let workspace_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap()
    .workspace_key("workspace-1")
    .unwrap();
    let table = if incomplete_transcript {
        "transcripts"
    } else {
        "sessions"
    };
    let mut events = (0..20)
        .map(|index| {
            let sealed = workspace_key
                .seal_field(
                    "workspace-1",
                    table,
                    &format!("session-{index:02}"),
                    "$row",
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    1,
                    false,
                    serde_json::json!(true),
                )
                .unwrap();
            anlg_db_app::E2eeWitnessEvent {
                sequence: u64::try_from(index + 1).unwrap(),
                record_id: sealed.record_id,
                workspace_id: "workspace-1".to_string(),
                payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
                payload: sealed.payload,
            }
        })
        .collect::<Vec<_>>();
    if incomplete_transcript {
        let sealed = workspace_key
            .seal_field(
                "workspace-1",
                table,
                "session-00",
                "words_json#n",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                1,
                false,
                serde_json::json!(1),
            )
            .unwrap();
        events.push(anlg_db_app::E2eeWitnessEvent {
            sequence: 21,
            record_id: sealed.record_id,
            workspace_id: "workspace-1".to_string(),
            payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
            payload: sealed.payload,
        });
    }
    anlg_db_app::merge_e2ee_witness_events(db.pool(), &workspace_key, "workspace-1", &events)
        .await
        .unwrap();
    let keys = HashMap::from([(
        "workspace-1".to_string(),
        anlg_e2ee::WorkspaceKeyring::new(workspace_key),
    )]);

    runtime
        .materialize_authenticated_e2ee_changes(
            &keys,
            &crate::e2ee_witness::E2eeWitnessCancellation::default(),
        )
        .await
        .unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i64>(if incomplete_transcript {
            "SELECT COUNT(*) FROM transcripts WHERE id LIKE 'session-%'"
        } else {
            "SELECT COUNT(*) FROM sessions WHERE id LIKE 'session-%'"
        })
        .fetch_one(db.pool())
        .await
        .unwrap(),
        20
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_replica_pending")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        i64::from(incomplete_transcript)
    );
}

#[tokio::test]
async fn witness_hydration_leaves_incomplete_transcripts_for_the_next_page() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = DesktopDbRuntime::<TestQueryEventSink>::for_test(std::sync::Arc::clone(&db));
    let key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap()
    .workspace_key("workspace-1")
    .unwrap();
    let words = serde_json::json!([{ "text": "restored", "start_ms": 0, "end_ms": 500 }]);
    let events = [
        ("$row", serde_json::json!(true)),
        ("words_json#n", serde_json::json!(1)),
        ("words_json#0", words.clone()),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (field, value))| {
        let sealed = key
            .seal_field(
                "workspace-1",
                "transcripts",
                "transcript-1",
                field,
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                1,
                false,
                value,
            )
            .unwrap();
        anlg_db_app::E2eeWitnessEvent {
            sequence: index as u64 + 1,
            record_id: sealed.record_id,
            workspace_id: "workspace-1".to_string(),
            payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
            payload: sealed.payload,
        }
    })
    .collect::<Vec<_>>();
    let keys = HashMap::from([("workspace-1".to_string(), key.clone().into())]);
    let cancellation = crate::e2ee_witness::E2eeWitnessCancellation::default();

    for page in [&events[..2], &events[2..]] {
        anlg_db_app::merge_e2ee_witness_events(db.pool(), &key, "workspace-1", page)
            .await
            .unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            runtime.materialize_authenticated_e2ee_changes(&keys, &cancellation),
        )
        .await
        .expect("hydration blocked the next witness page on an incomplete transcript")
        .unwrap();
        let actual: String =
            sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'transcript-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let expected = if page.len() == 2 {
            serde_json::json!([])
        } else {
            words.clone()
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&actual).unwrap(),
            expected
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_replica_pending")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn reconciliation_barrier_blocks_renderer_writes() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    let runtime = std::sync::Arc::new(DesktopDbRuntime::<TestQueryEventSink>::for_test(db));
    let guard = runtime.synced_write_barrier.write().await;

    let execute_runtime = std::sync::Arc::clone(&runtime);
    let mut execute = tokio::spawn(async move {
        execute_runtime
            .execute(
                "INSERT INTO sessions (id, title) VALUES ('session-1', 'Session 1')".to_string(),
                vec![],
            )
            .await
    });
    let transaction_runtime = std::sync::Arc::clone(&runtime);
    let mut transaction = tokio::spawn(async move {
        transaction_runtime
            .execute_transaction(vec![TransactionStatement {
                sql: "INSERT INTO sessions (id, title) VALUES ('session-2', 'Session 2')"
                    .to_string(),
                params: vec![],
                expected_rows_affected: Some(1),
            }])
            .await
    });
    let proxy_runtime = std::sync::Arc::clone(&runtime);
    let mut proxy = tokio::spawn(async move {
        proxy_runtime
            .execute_proxy(
                "INSERT INTO sessions (id, title) VALUES ('session-3', 'Session 3')".to_string(),
                vec![],
                ProxyQueryMethod::Run,
            )
            .await
    });

    let timeout = std::time::Duration::from_millis(25);
    assert!(tokio::time::timeout(timeout, &mut execute).await.is_err());
    assert!(
        tokio::time::timeout(timeout, &mut transaction)
            .await
            .is_err()
    );
    assert!(tokio::time::timeout(timeout, &mut proxy).await.is_err());
    drop(guard);

    execute.await.unwrap().unwrap();
    transaction.await.unwrap().unwrap();
    proxy.await.unwrap().unwrap();

    let session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(runtime.pool())
        .await
        .unwrap();
    assert_eq!(session_count, 3);
}

#[tokio::test]
async fn reconciliation_barrier_blocks_native_synced_writes() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    let runtime = std::sync::Arc::new(DesktopDbRuntime::<TestQueryEventSink>::for_test(db));
    let guard = runtime.synced_write_barrier.write().await;
    let write_runtime = std::sync::Arc::clone(&runtime);
    let mut write = tokio::spawn(async move {
        let _guard = write_runtime.synced_write_guard().await;
    });

    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(25), &mut write)
            .await
            .is_err()
    );
    drop(guard);
    write.await.unwrap();
}

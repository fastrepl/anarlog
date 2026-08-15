use super::*;

#[tokio::test]
async fn configures_replica_transport_without_the_cloudsync_extension() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    sqlx::query(
        "UPDATE storage_migration_state
         SET importer_version = ?, parity_verified = 1
         WHERE id = 'legacy_v1'",
    )
    .bind(anlg_db_app::LEGACY_IMPORTER_VERSION)
    .execute(db.pool())
    .await
    .unwrap();
    let runtime = PluginDbRuntime::new(db);
    let witness_state = InitiallyUninitializedWitness::default();
    witness_state.initialized.store(true, Ordering::SeqCst);
    let witness_server = MockServer::start().await;
    Mock::given(path("/sync/e2ee/witness/user-a"))
        .respond_with(witness_state)
        .mount(&witness_server)
        .await;
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let generation = runtime.begin_cloudsync_auth_configuration();

    assert_eq!(
        runtime
            .configure_replica_transport_at_generation(
                "user-a".to_string(),
                crate::CloudsyncE2eeWitness {
                    endpoint: format!("{}/sync/e2ee/witness/user-a", witness_server.uri()),
                    access_token: "access-token".to_string(),
                },
                recovery_key,
                generation,
            )
            .await
            .unwrap(),
        crate::CloudsyncTokenConfigurationResult::Configured
    );
    assert!(runtime.e2ee_sync_hook.replica_transport_configured());
    assert_eq!(
        runtime.cloudsync_status().await.unwrap()["configured"],
        true
    );
}

fn test_full_resync_config(token: &str) -> anlg_db_core::CloudsyncRuntimeConfig {
    anlg_db_core::CloudsyncRuntimeConfig {
        connection_string: "test-database".to_string(),
        auth: anlg_db_core::CloudsyncAuth::Token {
            token: token.to_string(),
        },
        tables: vec![],
        sync_interval_ms: DEFAULT_CLOUDSYNC_INTERVAL_MS,
        wait_ms: Some(5_000),
        max_retries: Some(3),
    }
}

async fn install_waiting_full_resync_task(
    runtime: &PluginDbRuntime,
) -> tokio::sync::oneshot::Receiver<()> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
    let join_handle = tokio::spawn(async move {
        let _ = shutdown_rx.await;
        let _ = finished_tx.send(());
    });
    runtime
        .scheduled_cloudsync_full_resync
        .lock()
        .unwrap()
        .claim("test-generation");
    *runtime.cloudsync_full_resync_task.lock().await = Some(CloudsyncFullResyncTask {
        config: test_full_resync_config("test-token"),
        generation: "test-generation".to_string(),
        shutdown_tx: Some(shutdown_tx),
        join_handle,
    });
    finished_rx
}

async fn install_blocked_full_resync_cancellation(
    runtime: &PluginDbRuntime,
) -> (
    tokio::sync::oneshot::Sender<()>,
    tokio::sync::oneshot::Receiver<()>,
) {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (cancellation_started_tx, cancellation_started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let join_handle = tokio::spawn(async move {
        let _ = shutdown_rx.await;
        let _ = cancellation_started_tx.send(());
        let _ = release_rx.await;
    });
    runtime
        .scheduled_cloudsync_full_resync
        .lock()
        .unwrap()
        .claim("test-generation");
    *runtime.cloudsync_full_resync_task.lock().await = Some(CloudsyncFullResyncTask {
        config: test_full_resync_config("test-token"),
        generation: "test-generation".to_string(),
        shutdown_tx: Some(shutdown_tx),
        join_handle,
    });
    (release_tx, cancellation_started_rx)
}

async fn assert_auth_invalidation_cancels_inflight_configuration(logout: bool) {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = std::sync::Arc::new(PluginDbRuntime::new(db));
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    runtime
        .set_e2ee_recovery_key("user-a", &recovery_key)
        .unwrap();
    let (release_cancellation, cancellation_started) =
        install_blocked_full_resync_cancellation(runtime.as_ref()).await;

    let configure_runtime = std::sync::Arc::clone(&runtime);
    let configure = tokio::spawn(async move {
        configure_runtime
            .configure_cloudsync_token(
                "managed-database-id".to_string(),
                "stale-token".to_string(),
                "user-a".to_string(),
                crate::CloudsyncE2eeWitness {
                    endpoint: "https://example.com/witness".to_string(),
                    access_token: "access-token".to_string(),
                },
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(1), cancellation_started)
        .await
        .expect("token configuration did not enter full-resync cancellation")
        .unwrap();
    let configuration_generation = runtime.cloudsync_auth_generation();

    let invalidation_runtime = std::sync::Arc::clone(&runtime);
    let invalidation = tokio::spawn(async move {
        if logout {
            invalidation_runtime.logout_cloudsync(false).await
        } else {
            invalidation_runtime.suspend_cloudsync().await
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while runtime.cloudsync_auth_generation() == configuration_generation {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("auth generation was not invalidated before waiting");

    release_cancellation
        .send(())
        .expect("failed to release full-resync cancellation");
    let error = tokio::time::timeout(std::time::Duration::from_secs(1), configure)
        .await
        .expect("cancelled token configuration did not finish")
        .unwrap()
        .unwrap_err();
    assert!(matches!(
        error,
        crate::Error::CloudsyncConfigurationCancelled
    ));
    tokio::time::timeout(std::time::Duration::from_secs(1), invalidation)
        .await
        .expect("CloudSync invalidation did not finish")
        .unwrap()
        .unwrap();

    assert!(runtime.workspace_key("user-a").is_none());
    assert!(runtime.cloudsync_full_resync_task.lock().await.is_none());
    let status = runtime.cloudsync_status().await.unwrap();
    assert_eq!(status["configured"], false);
    assert_eq!(status["network_initialized"], false);
}

#[tokio::test]
async fn suspend_invalidates_and_fails_closed_an_inflight_token_configuration() {
    assert_auth_invalidation_cancels_inflight_configuration(false).await;
}

#[tokio::test]
async fn logout_invalidates_and_fails_closed_an_inflight_token_configuration() {
    assert_auth_invalidation_cancels_inflight_configuration(true).await;
}

async fn spawn_stalled_witness_configuration() -> (
    tempfile::TempDir,
    std::sync::Arc<PluginDbRuntime>,
    MockServer,
    tokio::task::JoinHandle<Result<crate::CloudsyncTokenConfigurationResult>>,
) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await
    .unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    sqlx::query(
        "UPDATE storage_migration_state
             SET importer_version = ?, parity_verified = 1
             WHERE id = 'legacy_v1'",
    )
    .bind(anlg_db_app::LEGACY_IMPORTER_VERSION)
    .execute(db.pool())
    .await
    .unwrap();
    let runtime = std::sync::Arc::new(PluginDbRuntime::new(std::sync::Arc::new(db)));
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    runtime
        .set_e2ee_recovery_key("user-a", &recovery_key)
        .unwrap();
    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );
    let witness_server = MockServer::start().await;
    Mock::given(path("/sync/e2ee/witness/user-a"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_secs(60))
                .set_body_json(serde_json::json!({
                    "initialized": true,
                    "initializedAt": "2026-07-23T00:00:00Z",
                    "headSequence": 0,
                    "throughSequence": 0,
                    "nextAfterSequence": 0,
                    "events": [],
                })),
        )
        .mount(&witness_server)
        .await;
    let witness_endpoint = format!("{}/sync/e2ee/witness/user-a", witness_server.uri());

    let configure_runtime = std::sync::Arc::clone(&runtime);
    let configure = tokio::spawn(async move {
        configure_runtime
            .configure_cloudsync_token(
                "managed-database-id".to_string(),
                "stale-token".to_string(),
                "user-a".to_string(),
                crate::CloudsyncE2eeWitness {
                    endpoint: witness_endpoint,
                    access_token: "access-token".to_string(),
                },
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if !witness_server
                .received_requests()
                .await
                .expect("failed to inspect witness requests")
                .is_empty()
            {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("token configuration did not reach the witness");
    (dir, runtime, witness_server, configure)
}

#[tokio::test]
async fn suspend_cancels_a_stalled_witness_configuration() {
    let (_dir, runtime, _witness_server, configure) = spawn_stalled_witness_configuration().await;
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        runtime.suspend_cloudsync(),
    )
    .await
    .expect("suspension waited for the stalled witness request")
    .unwrap();
    let error = configure.await.unwrap().unwrap_err();
    assert!(matches!(
        error,
        crate::Error::CloudsyncConfigurationCancelled
    ));
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn suspend_drains_token_configuration_during_large_replica_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let db = std::sync::Arc::new(
        Db::open(DbOpenOptions {
            storage: DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(4),
        })
        .await
        .unwrap(),
    );
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    sqlx::query(
        "UPDATE storage_migration_state
             SET importer_version = ?, parity_verified = 1
             WHERE id = 'legacy_v1'",
    )
    .bind(anlg_db_app::LEGACY_IMPORTER_VERSION)
    .execute(db.pool())
    .await
    .unwrap();
    anlg_db_app::claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    anlg_db_app::set_cloudsync_personal_write_scope(db.pool(), "user-a")
        .await
        .unwrap();
    sqlx::query(
        "WITH RECURSIVE rows(id) AS (
               SELECT 1
               UNION ALL
               SELECT id + 1 FROM rows WHERE id < 100000
             )
             INSERT INTO e2ee_records (id, workspace_id, payload)
             SELECT
               printf('record-%d', id),
               'user-a',
               printf('ciphertext-%d', id)
             FROM rows",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        100_000
    );
    db.cloudsync_init(CLOUDSYNC_REPLICA_TABLE, None, None)
        .await
        .unwrap();
    db.cloudsync_set_filter(CLOUDSYNC_REPLICA_TABLE, CLOUDSYNC_WRITE_FILTER)
        .await
        .unwrap();

    let runtime = std::sync::Arc::new(PluginDbRuntime::new(std::sync::Arc::clone(&db)));
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    runtime
        .set_e2ee_recovery_key("user-a", &recovery_key)
        .unwrap();
    let key = runtime.workspace_key("user-a").unwrap();
    let generation = anlg_db_app::stage_cloudsync_poison_recovery(db.pool(), "user-a", "user-a")
        .await
        .unwrap();
    let recovery = anlg_db_app::ensure_cloudsync_recovery_state(
        db.pool(),
        &generation,
        "user-a",
        "user-a",
        &key,
    )
    .await
    .unwrap();
    assert_eq!(
        recovery.phase,
        anlg_db_app::CloudsyncRecoveryPhase::NeedFirstLogout
    );

    let witness_state = InitiallyUninitializedWitness::default();
    witness_state.initialized.store(true, Ordering::SeqCst);
    let witness_server = MockServer::start().await;
    Mock::given(path("/sync/e2ee/witness/user-a"))
        .respond_with(witness_state)
        .mount(&witness_server)
        .await;
    let witness_endpoint = format!("{}/sync/e2ee/witness/user-a", witness_server.uri());

    let configure_runtime = std::sync::Arc::clone(&runtime);
    let configure = tokio::spawn(async move {
        configure_runtime
            .configure_cloudsync_token(
                "managed-database-id".to_string(),
                "stale-token".to_string(),
                "user-a".to_string(),
                crate::CloudsyncE2eeWitness {
                    endpoint: witness_endpoint,
                    access_token: "access-token".to_string(),
                },
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !db.cloudsync_interrupt_registered() {
            assert!(
                !configure.is_finished(),
                "token configuration finished before native replica cleanup"
            );
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("token configuration did not register native replica cleanup");

    let suspended_at = std::time::Instant::now();
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        runtime.suspend_cloudsync(),
    )
    .await
    .expect("suspension did not drain native replica cleanup")
    .unwrap();
    assert!(
        suspended_at.elapsed() < std::time::Duration::from_secs(1),
        "suspension exceeded the cancellation deadline"
    );

    let error = configure.await.unwrap().unwrap_err();
    assert!(matches!(
        error,
        crate::Error::CloudsyncConfigurationCancelled
    ));
    assert!(!db.cloudsync_interrupt_registered());

    let idle_at = std::time::Instant::now();
    db.cloudsync_wait_for_sync_idle().await;
    assert!(
        idle_at.elapsed() < std::time::Duration::from_millis(250),
        "CloudSync worker was not idle after suspension"
    );
    db.cloudsync_version()
        .await
        .expect("CloudSync SQLite worker crashed during cancellation");
    let status = runtime.cloudsync_status().await.unwrap();
    assert_eq!(status["configured"], false);
    assert_eq!(status["running"], false);
    assert_eq!(status["network_initialized"], false);

    let inserted_at = std::time::Instant::now();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, title)
             VALUES ('local-after-cleanup-cancellation', 'user-a', 'Local')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        inserted_at.elapsed() < std::time::Duration::from_millis(250),
        "local write remained blocked after configuration cancellation"
    );
}

#[tokio::test]
async fn activity_begin_cancels_and_drains_a_stalled_witness_configuration() {
    let (_dir, runtime, _witness_server, configure) = spawn_stalled_witness_configuration().await;

    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        runtime.begin_cloudsync_activity_with_timeout(
            "capture".to_string(),
            "session-1".to_string(),
            std::time::Duration::from_millis(500),
        ),
    )
    .await
    .expect("activity begin waited for the stalled witness request")
    .unwrap();
    let error = configure.await.unwrap().unwrap_err();
    assert!(matches!(error, crate::Error::CloudsyncActivityDeferred));
    assert!(runtime.e2ee_sync_hook.activity_paused());
    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('activity-write', 'user-a', 'user-a', 'Local write')",
        )
        .execute(runtime.pool()),
    )
    .await
    .expect("local write remained blocked after configuration drained")
    .unwrap();
}

#[tokio::test]
async fn stale_configuration_waiting_after_a_newer_attempt_does_not_clear_its_key() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    let stale_generation = runtime.begin_cloudsync_auth_configuration();
    let _newer_generation = runtime.begin_cloudsync_auth_configuration();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    runtime
        .set_e2ee_recovery_key("new-user", &recovery_key)
        .unwrap();

    let error = runtime
        .configure_cloudsync_token_with_projection_at_generation(
            CloudsyncTokenConfiguration::new(
                "managed-database-id".to_string(),
                "stale-token".to_string(),
                "old-user".to_string(),
                None,
                crate::CloudsyncE2eeWitness {
                    endpoint: "https://example.com/witness".to_string(),
                    access_token: "access-token".to_string(),
                },
            ),
            Some(crate::runtime::E2eeWorkspaceKeyConfiguration::new(
                "old-user".to_string(),
                recovery_key,
                std::collections::HashMap::new(),
            )),
            stale_generation,
        )
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        crate::Error::CloudsyncConfigurationCancelled
    ));
    assert!(runtime.workspace_key("new-user").is_some());
    assert!(runtime.workspace_key("old-user").is_none());
}

#[tokio::test]
async fn suspend_joins_full_resync_before_returning() {
    let runtime = PluginDbRuntime::new(std::sync::Arc::new(
        Db::connect_memory_plain().await.unwrap(),
    ));
    let finished = install_waiting_full_resync_task(&runtime).await;

    runtime.suspend_cloudsync().await.unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(1), finished)
        .await
        .unwrap()
        .unwrap();
    assert!(runtime.cloudsync_full_resync_task.lock().await.is_none());
    assert!(
        !runtime
            .scheduled_cloudsync_full_resync
            .lock()
            .unwrap()
            .is_active("test-generation")
    );
}

#[tokio::test]
async fn suspend_cancels_recovery_before_waiting_for_its_control_guard() {
    let runtime = PluginDbRuntime::new(std::sync::Arc::new(
        Db::connect_memory_plain().await.unwrap(),
    ));
    let control_operation = std::sync::Arc::clone(&runtime.cloudsync_control_operation);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (control_acquired_tx, control_acquired_rx) = tokio::sync::oneshot::channel();
    let join_handle = tokio::spawn(async move {
        let _control_operation = control_operation.lock().await;
        let _ = control_acquired_tx.send(());
        let _ = shutdown_rx.await;
    });
    runtime
        .scheduled_cloudsync_full_resync
        .lock()
        .unwrap()
        .claim("test-generation");
    *runtime.cloudsync_full_resync_task.lock().await = Some(CloudsyncFullResyncTask {
        config: test_full_resync_config("test-token"),
        generation: "test-generation".to_string(),
        shutdown_tx: Some(shutdown_tx),
        join_handle,
    });
    tokio::time::timeout(std::time::Duration::from_secs(1), control_acquired_rx)
        .await
        .expect("recovery task did not acquire CloudSync control")
        .unwrap();

    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        runtime.suspend_cloudsync(),
    )
    .await
    .expect("suspension waited for recovery instead of cancelling it")
    .unwrap();
}

#[tokio::test]
async fn full_resync_cancellation_drains_while_control_is_held() {
    let runtime = PluginDbRuntime::new(std::sync::Arc::new(
        Db::connect_memory_plain().await.unwrap(),
    ));
    let control = runtime.cloudsync_control_operation.lock().await;
    runtime
        .schedule_cloudsync_full_resync(
            "test-generation".to_string(),
            test_full_resync_config("test-token"),
            runtime.cloudsync_auth_generation(),
        )
        .await;
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while runtime
            .e2ee_sync_hook
            .full_resync_control_waits
            .load(Ordering::Acquire)
            == 0
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("full resync did not wait for CloudSync control");

    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        runtime.cancel_cloudsync_full_resync(),
    )
    .await
    .expect("full resync cancellation deadlocked behind CloudSync control");
    assert!(runtime.cloudsync_full_resync_task.lock().await.is_none());
    drop(control);
}

#[tokio::test]
async fn logout_joins_full_resync_before_returning() {
    let runtime = PluginDbRuntime::new(std::sync::Arc::new(
        Db::connect_memory_plain().await.unwrap(),
    ));
    let finished = install_waiting_full_resync_task(&runtime).await;

    runtime.logout_cloudsync(false).await.unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(1), finished)
        .await
        .unwrap()
        .unwrap();
    assert!(runtime.cloudsync_full_resync_task.lock().await.is_none());
}

#[tokio::test]
async fn dropping_runtime_signals_full_resync_task_shutdown() {
    let runtime = PluginDbRuntime::new(std::sync::Arc::new(
        Db::connect_memory_plain().await.unwrap(),
    ));
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (shutdown_observed_tx, shutdown_observed_rx) = tokio::sync::oneshot::channel();
    let join_handle = tokio::spawn(async move {
        let _ = started_tx.send(());
        shutdown_rx.await.unwrap();
        let _ = shutdown_observed_tx.send(());
    });
    *runtime.cloudsync_full_resync_task.lock().await = Some(CloudsyncFullResyncTask {
        config: test_full_resync_config("test-token"),
        generation: "test-generation".to_string(),
        shutdown_tx: Some(shutdown_tx),
        join_handle,
    });
    started_rx.await.unwrap();

    drop(runtime);

    tokio::time::timeout(std::time::Duration::from_secs(1), shutdown_observed_rx)
        .await
        .unwrap()
        .unwrap();
}

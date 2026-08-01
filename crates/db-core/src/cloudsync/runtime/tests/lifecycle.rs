use super::super::*;
use super::test_cloudsync_config;
use crate::{CloudsyncAuth, CloudsyncTableSpec, DbOpenOptions, DbStorage};
use std::sync::atomic::{AtomicBool, Ordering};

#[tokio::test]
async fn logout_releases_connection_after_partial_startup() {
    let mut db = Db::connect_memory_plain().await.unwrap();
    db.cloudsync_enabled = true;
    db.cloudsync_configure(test_cloudsync_config())
        .await
        .unwrap();
    *db.cloudsync_connection.lock().await = Some(db.pool.acquire().await.unwrap());
    db.cloudsync_runtime.lock().unwrap().outbound_work_state = Some(false);

    db.cloudsync_logout(false).await.unwrap();

    assert!(db.cloudsync_connection.lock().await.is_none());
    let runtime = db.cloudsync_runtime.lock().unwrap();
    assert!(runtime.config.is_none());
    assert!(runtime.outbound_work_state.is_none());
}

#[tokio::test]
async fn logout_signals_shutdown_before_waiting_for_an_active_sync_operation() {
    let mut db = Db::connect_memory_plain().await.unwrap();
    db.cloudsync_enabled = true;
    db.cloudsync_configure(test_cloudsync_config())
        .await
        .unwrap();
    let db = Arc::new(db);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let (shutdown_observed_tx, shutdown_observed_rx) = oneshot::channel();
    let join_handle = tokio::spawn(async move {
        shutdown_rx.await.unwrap();
        let _ = shutdown_observed_tx.send(());
    });
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.running = true;
        runtime.task = Some(CloudsyncBackgroundTask {
            shutdown_tx: Some(shutdown_tx),
            join_handle,
        });
    }
    let sync_operation = db.cloudsync_sync_operation.lock().await;
    let logout_db = Arc::clone(&db);
    let logout = tokio::spawn(async move { logout_db.cloudsync_logout(false).await });

    tokio::time::timeout(Duration::from_secs(1), shutdown_observed_rx)
        .await
        .expect("logout waited for the sync operation before signaling shutdown")
        .unwrap();
    assert!(!logout.is_finished());

    drop(sync_operation);
    logout.await.unwrap().unwrap();
}

#[tokio::test]
async fn task_only_resume_reuses_initialized_transport_and_preserves_error_state() {
    let mut db = Db::connect_memory_plain().await.unwrap();
    db.cloudsync_enabled = true;
    let config = test_cloudsync_config();
    db.cloudsync_configure(config.clone()).await.unwrap();
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.network_initialized = true;
        runtime.last_error = Some("existing sync failure".to_string());
        runtime.consecutive_failures = 2;
    }
    let sync_operation = db.cloudsync_sync_operation.lock().await;

    db.start_cloudsync_background_task(&config);

    {
        let runtime = db.cloudsync_runtime.lock().unwrap();
        assert!(runtime.running);
        assert!(runtime.network_initialized);
        assert!(runtime.task.is_some());
        assert_eq!(runtime.last_error.as_deref(), Some("existing sync failure"));
        assert_eq!(runtime.consecutive_failures, 2);
    }
    db.stop_cloudsync_task().await;
    drop(sync_operation);
}

#[tokio::test]
async fn authentication_failure_cleans_up_initialized_network() {
    let cleanup_called = AtomicBool::new(false);

    let error = authenticate_cloudsync_network(
        || async {
            Err::<(), _>(anlg_cloudsync::Error::from(std::io::Error::other(
                "authentication rejected",
            )))
        },
        || async {
            cleanup_called.store(true, Ordering::SeqCst);
            Ok::<(), anlg_cloudsync::Error>(())
        },
    )
    .await
    .unwrap_err();

    assert!(cleanup_called.load(Ordering::SeqCst));
    assert!(error.to_string().contains("authentication rejected"));
}

#[tokio::test]
async fn manual_transport_initializes_without_starting_background_sync() {
    let db = Db::connect_memory().await.unwrap();
    sqlx::query("CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();
    let mut config = test_cloudsync_config();
    config.tables = vec![CloudsyncTableSpec {
        table_name: "items".to_string(),
        crdt_algo: None,
        init_flags: None,
        enabled: true,
    }];

    db.cloudsync_prepare_manual_transport(config).await.unwrap();

    let status = db.cloudsync_status().await.unwrap();
    assert!(status.configured);
    assert!(!status.running);
    assert!(status.network_initialized);
    assert!(db.cloudsync_runtime.lock().unwrap().task.is_none());
    let pending = db.cloudsync_manual_pending_payload_batch().await.unwrap();
    assert_eq!(pending.chunks, 0);
    assert!(pending.complete);
    assert!(pending.fits);
    db.cloudsync_suspend().await.unwrap();
}

#[tokio::test]
async fn manual_transport_refuses_to_take_over_a_running_runtime() {
    let db = Db::connect_memory().await.unwrap();
    db.cloudsync_runtime.lock().unwrap().running = true;

    let error = db
        .cloudsync_prepare_manual_transport(test_cloudsync_config())
        .await
        .unwrap_err();

    assert!(matches!(error, CloudsyncRuntimeError::RestartRequired));
    db.cloudsync_runtime.lock().unwrap().running = false;
}

#[tokio::test]
async fn prepared_transport_resumes_without_reinitializing_the_connection() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Local(&dir.path().join("app.db")),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(2),
    })
    .await
    .unwrap();
    let config = test_cloudsync_config();
    db.cloudsync_prepare_manual_transport(config).await.unwrap();
    {
        let mut connection = db.cloudsync_connection.lock().await;
        sqlx::query("CREATE TEMP TABLE transport_marker (value INTEGER)")
            .execute(&mut **connection.as_mut().unwrap())
            .await
            .unwrap();
    }
    let sync_operation = db.cloudsync_sync_operation.lock().await;
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.last_error = Some("stale recovery error".to_string());
        runtime.last_error_kind = Some(anlg_cloudsync::ErrorKind::Transient);
        runtime.consecutive_failures = 3;
        runtime.outbound_work_state = Some(true);
    }

    db.cloudsync_resume_prepared_transport().await.unwrap();

    let marker_exists: i64 = {
        let mut connection = db.cloudsync_connection.lock().await;
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_temp_master WHERE name = 'transport_marker'",
        )
        .fetch_one(&mut **connection.as_mut().unwrap())
        .await
        .unwrap()
    };
    assert_eq!(marker_exists, 1);
    let status = db.cloudsync_status().await.unwrap();
    assert!(status.running);
    assert!(status.network_initialized);
    assert!(status.last_error.is_none());
    assert!(status.last_error_kind.is_none());
    assert_eq!(status.consecutive_failures, 0);
    assert_eq!(status.has_unsent_changes, None);

    db.stop_cloudsync_task().await;
    drop(sync_operation);
    db.cloudsync_suspend().await.unwrap();
}

#[tokio::test]
async fn prepared_transport_reaps_a_finished_background_task_before_resuming() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Local(&dir.path().join("app.db")),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(2),
    })
    .await
    .unwrap();
    db.cloudsync_prepare_manual_transport(test_cloudsync_config())
        .await
        .unwrap();
    let sync_operation = db.cloudsync_sync_operation.lock().await;
    let join_handle = tokio::spawn(async {});
    while !join_handle.is_finished() {
        tokio::task::yield_now().await;
    }
    let (shutdown_tx, _shutdown_rx) = oneshot::channel();
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.running = false;
        runtime.task = Some(CloudsyncBackgroundTask {
            shutdown_tx: Some(shutdown_tx),
            join_handle,
        });
    }

    db.cloudsync_resume_prepared_transport().await.unwrap();

    {
        let runtime = db.cloudsync_runtime.lock().unwrap();
        assert!(runtime.running);
        assert!(runtime.network_initialized);
        assert!(runtime.task.is_some());
    }

    db.stop_cloudsync_task().await;
    drop(sync_operation);
    db.cloudsync_suspend().await.unwrap();
}

#[tokio::test]
async fn configure_rejects_mutation_while_manual_transport_is_prepared() {
    let db = Db::connect_memory().await.unwrap();
    let original = test_cloudsync_config();
    db.cloudsync_prepare_manual_transport(original.clone())
        .await
        .unwrap();
    let mut replacement = original.clone();
    replacement.connection_string = "replacement-managed-database-id".to_string();

    let error = db.cloudsync_configure(replacement).await.unwrap_err();

    assert!(matches!(error, CloudsyncRuntimeError::RestartRequired));
    assert_eq!(
        db.cloudsync_runtime.lock().unwrap().config.as_ref(),
        Some(&original)
    );
    db.cloudsync_suspend().await.unwrap();
}

#[tokio::test]
async fn reconfigure_cleans_up_a_prepared_manual_transport() {
    let db = Db::connect_memory().await.unwrap();
    db.cloudsync_prepare_manual_transport(test_cloudsync_config())
        .await
        .unwrap();
    let mut replacement = test_cloudsync_config();
    replacement.connection_string = "replacement-managed-database-id".to_string();

    db.cloudsync_reconfigure(replacement.clone()).await.unwrap();

    let status = db.cloudsync_status().await.unwrap();
    assert!(status.configured);
    assert!(!status.running);
    assert!(!status.network_initialized);
    assert_eq!(
        db.cloudsync_runtime.lock().unwrap().config.as_ref(),
        Some(&replacement)
    );
    db.cloudsync_suspend().await.unwrap();
}

#[tokio::test]
async fn configure_start_and_suspend_transitions_are_serialized() {
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Memory,
        cloudsync_enabled: false,
        journal_mode_wal: false,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();
    db.cloudsync_configure(CloudsyncRuntimeConfig {
        connection_string: "managed-database-id".to_string(),
        auth: CloudsyncAuth::None,
        tables: Vec::new(),
        sync_interval_ms: 30_000,
        wait_ms: Some(5_000),
        max_retries: Some(3),
    })
    .await
    .unwrap();

    let lifecycle = db.cloudsync_lifecycle.lock().await;
    let mut configure = Box::pin(db.cloudsync_configure(CloudsyncRuntimeConfig {
        connection_string: "next-managed-database-id".to_string(),
        auth: CloudsyncAuth::None,
        tables: Vec::new(),
        sync_interval_ms: 45_000,
        wait_ms: Some(5_000),
        max_retries: Some(3),
    }));
    tokio::select! {
        biased;
        result = &mut configure => panic!("configure bypassed lifecycle lock: {result:?}"),
        _ = tokio::task::yield_now() => {}
    }

    let mut start = Box::pin(db.cloudsync_start());
    tokio::select! {
        biased;
        result = &mut start => panic!("start bypassed lifecycle lock: {result:?}"),
        _ = tokio::task::yield_now() => {}
    }

    let mut suspend = Box::pin(db.cloudsync_suspend());
    tokio::select! {
        biased;
        result = &mut suspend => panic!("suspend bypassed lifecycle lock: {result:?}"),
        _ = tokio::task::yield_now() => {}
    }

    drop(lifecycle);
    configure.await.unwrap();
    start.await.unwrap();
    suspend.await.unwrap();

    let status = db.cloudsync_status().await.unwrap();
    assert!(!status.configured);
    assert!(!status.running);
    assert!(!status.network_initialized);
}

#[tokio::test]
async fn status_stays_observable_while_mutations_wait_for_lifecycle() {
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Memory,
        cloudsync_enabled: false,
        journal_mode_wal: false,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();
    db.cloudsync_configure(CloudsyncRuntimeConfig {
        connection_string: "managed-database-id".to_string(),
        auth: CloudsyncAuth::None,
        tables: Vec::new(),
        sync_interval_ms: 30_000,
        wait_ms: Some(5_000),
        max_retries: Some(3),
    })
    .await
    .unwrap();

    let lifecycle = db.cloudsync_lifecycle.lock().await;
    let mut suspend = Box::pin(db.cloudsync_suspend());
    tokio::select! {
        biased;
        result = &mut suspend => panic!("suspend bypassed lifecycle lock: {result:?}"),
        _ = tokio::task::yield_now() => {}
    }

    let status = tokio::time::timeout(Duration::from_millis(100), db.cloudsync_status())
        .await
        .expect("status blocked on the lifecycle lock")
        .unwrap();
    assert!(status.configured);
    assert!(!status.running);
    assert_eq!(status.has_unsent_changes, None);

    let mut trigger = Box::pin(db.cloudsync_trigger_sync());
    tokio::select! {
        biased;
        result = &mut trigger => panic!("manual sync bypassed lifecycle lock: {result:?}"),
        _ = tokio::task::yield_now() => {}
    }

    drop(lifecycle);
    suspend.await.unwrap();
    assert_eq!(trigger.await.unwrap(), CloudsyncNetworkResult::default());
    assert!(!db.cloudsync_status().await.unwrap().configured);
}

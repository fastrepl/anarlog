use super::super::*;
use crate::{CloudsyncAuth, CloudsyncTableSpec, DbOpenOptions, DbStorage};
use std::future::pending;

#[tokio::test]
async fn restart_after_fatal_exit_cleans_native_state() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("app.db");
    let db = Db::open(crate::DbOpenOptions {
        storage: crate::DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(2),
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
    db.cloudsync_start().await.unwrap();
    {
        let mut connection = db.cloudsync_connection.lock().await;
        sqlx::query("CREATE TEMP TABLE stale_cloudsync_connection (id INTEGER)")
            .execute(&mut **connection.as_mut().unwrap())
            .await
            .unwrap();
    }

    let mut running_task = db.cloudsync_runtime.lock().unwrap().task.take().unwrap();
    let _ = running_task.shutdown_tx.take().unwrap().send(());
    let _ = running_task.join_handle.await;

    let (stale_shutdown_tx, stale_shutdown_rx) = oneshot::channel::<()>();
    let (finished_tx, finished_rx) = oneshot::channel();
    let join_handle = tokio::spawn(async move {
        drop(stale_shutdown_rx);
        let _ = finished_tx.send(());
    });
    finished_rx.await.unwrap();
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.running = false;
        runtime.last_error = Some("fatal sync failure".to_string());
        runtime.last_error_kind = Some(anlg_cloudsync::ErrorKind::Fatal);
        runtime.task = Some(CloudsyncBackgroundTask {
            shutdown_tx: Some(stale_shutdown_tx),
            join_handle,
        });
    }

    db.cloudsync_start().await.unwrap();

    {
        let runtime = db.cloudsync_runtime.lock().unwrap();
        assert!(runtime.running);
        assert!(runtime.network_initialized);
        assert!(runtime.task.is_some());
        assert!(runtime.last_error.is_none());
    }
    let marker_count: i64 = {
        let mut connection = db.cloudsync_connection.lock().await;
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_temp_master WHERE name = 'stale_cloudsync_connection'",
        )
        .fetch_one(&mut **connection.as_mut().unwrap())
        .await
        .unwrap()
    };
    assert_eq!(marker_count, 0);
    db.cloudsync_stop().await.unwrap();
}

#[tokio::test]
async fn suspend_interrupts_active_retry_backoff() {
    let db = Db::connect_memory_plain().await.unwrap();
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    let (retry_started_tx, retry_started_rx) = oneshot::channel();
    let join_handle = tokio::spawn(async move {
        let sync_requested = tokio::sync::Notify::new();
        let _ = retry_started_tx.send(());
        assert!(
            !wait_for_retry_request_or_shutdown(
                Duration::from_secs(60),
                &sync_requested,
                &mut shutdown_rx,
            )
            .await
        );
    });
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.running = true;
        runtime.task = Some(CloudsyncBackgroundTask {
            shutdown_tx: Some(shutdown_tx),
            join_handle,
        });
    }
    retry_started_rx.await.unwrap();

    tokio::time::timeout(Duration::from_secs(1), db.cloudsync_suspend())
        .await
        .expect("suspend waited for retry backoff")
        .unwrap();

    assert!(!db.cloudsync_status().await.unwrap().running);
}

#[tokio::test]
async fn shutdown_interrupts_an_active_sync_future() {
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    shutdown_tx.send(()).unwrap();

    let result = tokio::time::timeout(
        Duration::from_secs(1),
        run_or_shutdown(pending::<()>(), &mut shutdown_rx),
    )
    .await
    .expect("active sync ignored shutdown");

    assert!(result.is_none());
}

#[tokio::test]
async fn suspend_stops_runtime_and_clears_in_memory_credentials() {
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
        auth: CloudsyncAuth::Token {
            token: "secret-token".to_string(),
        },
        tables: vec![CloudsyncTableSpec {
            table_name: "sessions".to_string(),
            crdt_algo: None,
            init_flags: None,
            enabled: true,
        }],
        sync_interval_ms: 30_000,
        wait_ms: Some(5_000),
        max_retries: Some(3),
    })
    .await
    .unwrap();

    db.cloudsync_start().await.unwrap();
    db.cloudsync_runtime.lock().unwrap().outbound_work_state = Some(false);
    db.cloudsync_suspend().await.unwrap();

    let status = db.cloudsync_status().await.unwrap();
    assert!(!status.configured);
    assert!(!status.running);
    assert!(!status.network_initialized);
    assert!(
        db.cloudsync_runtime
            .lock()
            .unwrap()
            .outbound_work_state
            .is_none()
    );
}

#[tokio::test]
async fn suspend_clears_runtime_state_when_native_teardown_fails() {
    let db = Db::connect_memory().await.unwrap();
    db.cloudsync_configure(CloudsyncRuntimeConfig {
        connection_string: "managed-database-id".to_string(),
        auth: CloudsyncAuth::Token {
            token: "secret-token".to_string(),
        },
        tables: Vec::new(),
        sync_interval_ms: 30_000,
        wait_ms: Some(5_000),
        max_retries: Some(3),
    })
    .await
    .unwrap();
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.running = true;
        runtime.network_initialized = true;
    }
    db.pool().close().await;

    db.cloudsync_suspend().await.unwrap_err();

    let status = db.cloudsync_status().await.unwrap();
    assert!(!status.configured);
    assert!(!status.running);
    assert!(!status.network_initialized);
    assert!(db.cloudsync_connection.lock().await.is_none());
}

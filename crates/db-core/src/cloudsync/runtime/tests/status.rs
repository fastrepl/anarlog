use super::super::*;
use super::{db_with_local_unsent_changes, test_cloudsync_config};

#[tokio::test]
async fn status_preserves_unknown_outbound_state_during_sync_preflight() {
    let db = Db::connect_memory().await.unwrap();
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.config = Some(test_cloudsync_config());
        runtime.running = true;
        runtime.network_initialized = true;
    }
    let _sync_operation = db.cloudsync_sync_operation.lock().await;

    let status = tokio::time::timeout(Duration::from_millis(100), db.cloudsync_status())
        .await
        .expect("status blocked on the active sync operation")
        .unwrap();

    assert!(status.configured);
    assert!(status.running);
    assert!(status.network_initialized);
    assert_eq!(status.has_unsent_changes, None);
}

#[tokio::test]
async fn status_reports_receive_only_work_while_a_sync_operation_is_running() {
    let db = Db::connect_memory().await.unwrap();
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.config = Some(test_cloudsync_config());
        runtime.running = true;
        runtime.network_initialized = true;
        runtime.last_sync_at_ms = Some(42);
        runtime.outbound_work_state = Some(false);
    }
    let _sync_operation = db.cloudsync_sync_operation.lock().await;

    let status = tokio::time::timeout(Duration::from_millis(100), db.cloudsync_status())
        .await
        .expect("status blocked on the active receive")
        .unwrap();

    assert_eq!(status.has_unsent_changes, Some(false));
    assert_eq!(status.last_sync_at_ms, Some(42));
}

#[tokio::test]
async fn status_reports_outbound_work_while_a_sync_operation_is_running() {
    let db = Db::connect_memory().await.unwrap();
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.config = Some(test_cloudsync_config());
        runtime.running = true;
        runtime.network_initialized = true;
        runtime.last_sync_at_ms = Some(42);
        runtime.outbound_work_state = Some(true);
    }
    let _sync_operation = db.cloudsync_sync_operation.lock().await;

    let status = tokio::time::timeout(Duration::from_millis(100), db.cloudsync_status())
        .await
        .expect("status blocked on the active send")
        .unwrap();

    assert_eq!(status.has_unsent_changes, Some(true));
}

#[tokio::test]
async fn status_reads_unsent_changes_without_network_io() {
    let (_dir, db) = db_with_local_unsent_changes().await;
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.config = Some(test_cloudsync_config());
        runtime.running = true;
        runtime.network_initialized = true;
    }

    let status = tokio::time::timeout(Duration::from_millis(100), db.cloudsync_status())
        .await
        .expect("local cloudsync status blocked")
        .unwrap();

    assert_eq!(status.has_unsent_changes, Some(true));
}

#[tokio::test]
async fn repeated_status_polling_does_not_queue_work_on_a_busy_pool() {
    let (_dir, db) = db_with_local_unsent_changes().await;
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.config = Some(test_cloudsync_config());
        runtime.running = true;
        runtime.network_initialized = true;
    }
    let mut first_connection = db.pool().acquire().await.unwrap();

    for _ in 0..32 {
        let status = tokio::time::timeout(Duration::from_millis(50), db.cloudsync_status())
            .await
            .expect("CloudSync status waited for a busy pool")
            .unwrap();
        assert_eq!(status.has_unsent_changes, None);
    }

    first_connection.return_to_pool().await;
    for _ in 0..32 {
        let status = tokio::time::timeout(Duration::from_millis(100), db.cloudsync_status())
            .await
            .expect("CloudSync status left SQLite work in flight")
            .unwrap();
        assert_eq!(status.has_unsent_changes, Some(true));
    }
    assert!(db.pool().num_idle() > 0);
    tokio::time::timeout(Duration::from_millis(100), db.pool().acquire())
        .await
        .expect("repeated CloudSync status polling exhausted the pool")
        .unwrap();
}

#[tokio::test]
async fn logout_checks_unsent_changes_without_network_io() {
    let (_dir, db) = db_with_local_unsent_changes().await;
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.config = Some(test_cloudsync_config());
        runtime.network_initialized = true;
    }

    let error = db.cloudsync_logout(false).await.unwrap_err();

    assert!(
        matches!(error, CloudsyncRuntimeError::UnsentChanges),
        "{error:?}"
    );
}

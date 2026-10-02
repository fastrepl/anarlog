use super::super::*;
use super::{db_with_local_unsent_changes, test_cloudsync_config};

#[tokio::test]
async fn status_reports_outbound_state_while_a_sync_operation_is_running() {
    for (
        outbound_work_state,
        expected_has_unsent_changes,
        last_sync_at_ms,
        expected_last_sync_at_ms,
    ) in [
        (None, None, None, None),
        (Some(false), Some(false), Some(42), Some(42)),
        (Some(true), Some(true), None, None),
    ] {
        let db = Db::connect_memory().await.unwrap();
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.config = Some(test_cloudsync_config());
            runtime.running = true;
            runtime.network_initialized = true;
            runtime.last_sync_at_ms = last_sync_at_ms;
            runtime.outbound_work_state = outbound_work_state;
        }
        let _sync_operation = db.cloudsync_sync_operation.lock().await;

        let status = tokio::time::timeout(Duration::from_millis(100), db.cloudsync_status())
            .await
            .expect("status blocked on the active sync operation")
            .unwrap();

        assert!(status.configured);
        assert!(status.running);
        assert!(status.network_initialized);
        assert_eq!(status.has_unsent_changes, expected_has_unsent_changes);
        assert_eq!(status.last_sync_at_ms, expected_last_sync_at_ms);
    }
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

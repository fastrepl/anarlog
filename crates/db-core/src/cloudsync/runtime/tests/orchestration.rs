use super::super::*;
use super::test_cloudsync_config;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[test]
fn background_sync_starts_immediately_and_continues_incomplete_receive_promptly() {
    let interval = Duration::from_secs(30);
    let incomplete = CloudsyncNetworkResult {
        send: None,
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 1,
            tables: vec!["e2ee_records".to_string()],
            chunks: 1,
            bytes: 1024,
            complete: false,
            error: None,
            last_failure: None,
        }),
    };

    assert_eq!(cloudsync_next_delay(None, false, interval), Duration::ZERO);
    assert_eq!(
        cloudsync_next_delay(Some(&incomplete), false, interval),
        CLOUDSYNC_PROGRESS_INTERVAL
    );

    let send_in_progress = CloudsyncNetworkResult {
        send: Some(anlg_cloudsync::NetworkSendResult {
            status: "syncing".to_string(),
            local_version: 3,
            server_version: 2,
            chunks: 1,
            bytes: 1024,
            last_failure: None,
        }),
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 0,
            tables: Vec::new(),
            chunks: 0,
            bytes: 0,
            complete: true,
            error: None,
            last_failure: None,
        }),
    };
    assert_eq!(
        cloudsync_next_delay(Some(&send_in_progress), false, interval),
        CLOUDSYNC_PROGRESS_INTERVAL
    );

    let settled = CloudsyncNetworkResult {
        send: Some(anlg_cloudsync::NetworkSendResult {
            status: "synced".to_string(),
            local_version: 3,
            server_version: 3,
            chunks: 0,
            bytes: 0,
            last_failure: None,
        }),
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 0,
            tables: Vec::new(),
            chunks: 0,
            bytes: 0,
            complete: true,
            error: None,
            last_failure: None,
        }),
    };
    assert_eq!(
        cloudsync_next_delay(Some(&settled), true, interval),
        CLOUDSYNC_PROGRESS_INTERVAL
    );

    let uploaded = CloudsyncNetworkResult {
        send: Some(anlg_cloudsync::NetworkSendResult {
            status: "synced".to_string(),
            local_version: 4,
            server_version: 4,
            chunks: 1,
            bytes: 1024,
            last_failure: None,
        }),
        receive: settled.receive,
    };
    assert_eq!(
        cloudsync_next_delay(Some(&uploaded), true, interval),
        interval
    );
}

#[derive(Default)]
struct RecordingSyncHook {
    directive: crate::CloudsyncSyncDirective,
    local_work_remaining: bool,
    activity_paused: AtomicBool,
    before_calls: AtomicUsize,
    after_result: Mutex<Option<CloudsyncNetworkResult>>,
}

impl crate::CloudsyncSyncHook for RecordingSyncHook {
    fn activity_paused(&self) -> bool {
        self.activity_paused.load(Ordering::SeqCst)
    }

    fn before_sync<'a>(&'a self, _pool: &'a SqlitePool) -> crate::CloudsyncBeforeHookFuture<'a> {
        Box::pin(async move {
            self.before_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.directive)
        })
    }

    fn after_sync<'a>(
        &'a self,
        _pool: &'a SqlitePool,
        result: &'a CloudsyncNetworkResult,
    ) -> crate::CloudsyncHookFuture<'a> {
        Box::pin(async move {
            *self.after_result.lock().unwrap() = Some(result.clone());
            Ok(crate::CloudsyncHookOutcome {
                local_work_remaining: self.local_work_remaining,
                deferred: false,
            })
        })
    }
}

#[tokio::test]
async fn pending_payload_preflight_skips_clean_databases() {
    let db = Db::connect_memory().await.unwrap();
    sqlx::query("CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();
    db.cloudsync_init("items", None, None).await.unwrap();
    let mut connection = db.pool().acquire().await.unwrap();

    assert!(
        !pending_cloudsync_payload_exists(&mut connection, &db.cloudsync_interrupt)
            .await
            .unwrap()
    );

    sqlx::query("INSERT INTO items (id) VALUES ('pending')")
        .execute(&mut *connection)
        .await
        .unwrap();
    assert!(
        pending_cloudsync_payload_exists(&mut connection, &db.cloudsync_interrupt)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn before_sync_hook_can_select_receive_only_transport() {
    let db = Db::connect_memory_plain().await.unwrap();
    let recording_hook = Arc::new(RecordingSyncHook {
        directive: crate::CloudsyncSyncDirective::ReceiveOnly,
        ..Default::default()
    });
    let hook: Arc<dyn crate::CloudsyncSyncHook> = recording_hook;
    let hook = Mutex::new(Some(hook));

    assert_eq!(
        run_before_sync_hook(&hook, db.pool()).await.unwrap(),
        crate::CloudsyncSyncDirective::ReceiveOnly
    );
}

#[tokio::test]
async fn deferred_before_sync_hook_never_starts_native_transport() {
    let db = Db::connect_memory().await.unwrap();
    sqlx::query("CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();
    db.cloudsync_init("items", None, None).await.unwrap();
    let recording_hook = Arc::new(RecordingSyncHook {
        directive: crate::CloudsyncSyncDirective::Deferred,
        ..Default::default()
    });
    let hook: Arc<dyn crate::CloudsyncSyncHook> = recording_hook.clone();
    let hook = Mutex::new(Some(hook));

    let result = sync_cloudsync_connection(
        db.pool(),
        &db.cloudsync_connection,
        &db.cloudsync_interrupt,
        &db.cloudsync_sync_operation,
        &db.cloudsync_runtime,
        &hook,
    )
    .await
    .unwrap();

    assert!(matches!(result, CloudsyncStepOutcome::Deferred));
    assert_eq!(recording_hook.before_calls.load(Ordering::SeqCst), 1);
    assert!(recording_hook.after_result.lock().unwrap().is_none());
}

#[tokio::test]
async fn existing_native_pending_batch_skips_before_sync_hook() {
    let db = Db::connect_memory().await.unwrap();
    sqlx::query(
        "CREATE TABLE items (
            id TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db.cloudsync_init("items", None, None).await.unwrap();
    sqlx::query("INSERT INTO items (id, value) VALUES ('item', 'pending')")
        .execute(db.pool())
        .await
        .unwrap();
    let recording_hook = Arc::new(RecordingSyncHook::default());
    let hook: Arc<dyn crate::CloudsyncSyncHook> = recording_hook.clone();
    let hook = Mutex::new(Some(hook));

    let result = sync_cloudsync_connection(
        db.pool(),
        &db.cloudsync_connection,
        &db.cloudsync_interrupt,
        &db.cloudsync_sync_operation,
        &db.cloudsync_runtime,
        &hook,
    )
    .await;
    let Err(error) = result else {
        panic!("sync unexpectedly succeeded");
    };

    assert_eq!(recording_hook.before_calls.load(Ordering::SeqCst), 0);
    assert!(
        error
            .to_string()
            .contains("Unable to retrieve CloudSync network context")
    );
}

#[tokio::test]
async fn activity_pause_precedes_an_existing_native_pending_batch() {
    let db = Db::connect_memory().await.unwrap();
    sqlx::query(
        "CREATE TABLE items (
            id TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db.cloudsync_init("items", None, None).await.unwrap();
    sqlx::query("INSERT INTO items (id, value) VALUES ('item', 'pending')")
        .execute(db.pool())
        .await
        .unwrap();
    let pending_before = {
        let mut connection = db.pool().acquire().await.unwrap();
        crate::cloudsync::ops::ensure_pending_payload_fits(&mut connection, &db.cloudsync_interrupt)
            .await
            .unwrap()
    };
    assert!(pending_before.chunks > 0);

    let recording_hook = Arc::new(RecordingSyncHook {
        activity_paused: AtomicBool::new(true),
        ..Default::default()
    });
    let hook: Arc<dyn crate::CloudsyncSyncHook> = recording_hook.clone();
    let hook = Mutex::new(Some(hook));
    let last_sync = CloudsyncNetworkResult {
        send: Some(anlg_cloudsync::NetworkSendResult {
            status: "synced".to_string(),
            local_version: 4,
            server_version: 4,
            chunks: 1,
            bytes: 1024,
            last_failure: None,
        }),
        receive: None,
    };
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.last_sync = Some(last_sync.clone());
        runtime.last_sync_at_ms = Some(42);
        runtime.last_error = Some("previous error".to_string());
    }

    let result = sync_cloudsync_connection(
        db.pool(),
        &db.cloudsync_connection,
        &db.cloudsync_interrupt,
        &db.cloudsync_sync_operation,
        &db.cloudsync_runtime,
        &hook,
    )
    .await;

    assert!(matches!(result, Ok(CloudsyncStepOutcome::Deferred)));
    assert_eq!(recording_hook.before_calls.load(Ordering::SeqCst), 0);
    assert!(recording_hook.after_result.lock().unwrap().is_none());
    let pending_after = {
        let mut connection = db.pool().acquire().await.unwrap();
        crate::cloudsync::ops::ensure_pending_payload_fits(&mut connection, &db.cloudsync_interrupt)
            .await
            .unwrap()
    };
    assert_eq!(pending_after, pending_before);
    let runtime = db.cloudsync_runtime.lock().unwrap();
    assert_eq!(runtime.last_sync.as_ref(), Some(&last_sync));
    assert_eq!(runtime.last_sync_at_ms, Some(42));
    assert_eq!(runtime.last_error.as_deref(), Some("previous error"));
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[tokio::test]
async fn activity_pause_during_pending_preflight_defers_and_drains_before_local_write() {
    let db = Arc::new(Db::connect_memory().await.unwrap());
    sqlx::query(
        "CREATE TABLE items (
            id TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db.cloudsync_init("items", None, None).await.unwrap();
    sqlx::query("SELECT cloudsync_set('payload_max_chunk_size', '33554432')")
        .fetch_optional(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "WITH RECURSIVE rows(id) AS (
            SELECT 1
            UNION ALL
            SELECT id + 1 FROM rows WHERE id < 100000
        )
        INSERT INTO items (id, value)
        SELECT printf('item-%d', id), printf('value-%d', id) FROM rows",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let recording_hook = Arc::new(RecordingSyncHook::default());
    db.set_cloudsync_sync_hook(recording_hook.clone());
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.config = Some(test_cloudsync_config());
        runtime.running = true;
        runtime.network_initialized = true;
    }

    let request_db = Arc::clone(&db);
    let request = tokio::spawn(async move { request_db.cloudsync_trigger_sync().await });
    let started_at = std::time::Instant::now();
    while !db.cloudsync_interrupt_registered() {
        assert!(
            !request.is_finished(),
            "pending payload generation completed before interrupt registration was observable"
        );
        assert!(
            started_at.elapsed() < Duration::from_secs(2),
            "pending payload generation never registered for interruption"
        );
        tokio::task::yield_now().await;
    }

    recording_hook.activity_paused.store(true, Ordering::SeqCst);
    while !request.is_finished() {
        db.cloudsync_interrupt_sync();
        assert!(
            started_at.elapsed() < Duration::from_secs(2),
            "pending payload generation did not honor sqlite3_interrupt"
        );
        tokio::task::yield_now().await;
    }

    assert_eq!(
        request.await.unwrap().unwrap(),
        CloudsyncNetworkResult::default()
    );
    assert!(db.cloudsync_runtime.lock().unwrap().last_error.is_none());
    assert_eq!(recording_hook.before_calls.load(Ordering::SeqCst), 0);
    assert!(recording_hook.after_result.lock().unwrap().is_none());
    assert!(!db.cloudsync_interrupt_registered());
    tokio::time::timeout(
        Duration::from_millis(250),
        db.cloudsync_wait_for_sync_idle(),
    )
    .await
    .expect("pending payload worker did not become idle after activity pause");
    tokio::time::timeout(
        Duration::from_millis(250),
        sqlx::query("INSERT INTO items (id, value) VALUES ('after', 'local')").execute(db.pool()),
    )
    .await
    .expect("immediate local write remained blocked after activity pause")
    .unwrap();
}

#[tokio::test]
async fn paused_manual_sync_preserves_the_last_result_and_error() {
    let db = Db::connect_memory().await.unwrap();
    let hook = Arc::new(RecordingSyncHook {
        activity_paused: AtomicBool::new(true),
        ..Default::default()
    });
    db.set_cloudsync_sync_hook(hook);
    db.cloudsync_configure(test_cloudsync_config())
        .await
        .unwrap();
    let last_sync = CloudsyncNetworkResult {
        send: None,
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 3,
            tables: vec!["sessions".to_string()],
            chunks: 1,
            bytes: 2048,
            complete: true,
            error: None,
            last_failure: None,
        }),
    };
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.network_initialized = true;
        runtime.last_sync = Some(last_sync.clone());
        runtime.last_sync_at_ms = Some(42);
        runtime.last_error = Some("previous error".to_string());
        runtime.consecutive_failures = 2;
    }

    assert_eq!(
        db.cloudsync_trigger_sync().await.unwrap(),
        CloudsyncNetworkResult::default()
    );
    let status = db.cloudsync_status().await.unwrap();
    assert!(status.activity_paused);
    assert_eq!(status.has_unsent_changes, None);
    assert_eq!(status.last_sync.as_ref(), Some(&last_sync));
    assert_eq!(status.last_sync_at_ms, Some(42));
    assert_eq!(status.last_error.as_deref(), Some("previous error"));
    assert_eq!(status.consecutive_failures, 2);
}

#[tokio::test]
async fn sync_idle_barrier_waits_for_the_active_operation() {
    let db = Db::connect_memory_plain().await.unwrap();
    let operation = db.cloudsync_sync_operation.lock().await;
    let mut barrier = Box::pin(db.cloudsync_wait_for_sync_idle());

    tokio::select! {
        biased;
        () = &mut barrier => panic!("sync idle barrier bypassed the active operation"),
        _ = tokio::task::yield_now() => {}
    }

    drop(operation);
    tokio::time::timeout(Duration::from_millis(100), barrier)
        .await
        .expect("sync idle barrier did not finish");
}

#[tokio::test]
async fn requested_sync_wakes_are_coalesced() {
    let db = Db::connect_memory_plain().await.unwrap();

    db.cloudsync_request_sync();
    assert!(
        tokio::time::timeout(
            Duration::from_millis(10),
            db.cloudsync_sync_requested.notified(),
        )
        .await
        .is_err(),
        "a stopped runtime retained a requested sync wake"
    );
    db.cloudsync_runtime.lock().unwrap().running = true;
    db.cloudsync_request_sync();
    db.cloudsync_request_sync();
    tokio::time::timeout(
        Duration::from_millis(100),
        db.cloudsync_sync_requested.notified(),
    )
    .await
    .expect("queued sync wake was lost");
    assert!(
        tokio::time::timeout(
            Duration::from_millis(10),
            db.cloudsync_sync_requested.notified(),
        )
        .await
        .is_err(),
        "duplicate sync wakes were not coalesced"
    );
}

#[tokio::test]
async fn requested_sync_interrupts_and_consumes_the_retry_backoff() {
    let sync_requested = tokio::sync::Notify::new();
    let (_shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

    sync_requested.notify_one();
    sync_requested.notify_one();
    tokio::time::timeout(
        Duration::from_millis(100),
        wait_for_retry_request_or_shutdown(
            Duration::from_secs(60),
            &sync_requested,
            &mut shutdown_rx,
        ),
    )
    .await
    .expect("requested sync did not interrupt retry backoff");
    assert!(
        tokio::time::timeout(
            Duration::from_millis(10),
            wait_for_retry_request_or_shutdown(
                Duration::from_secs(60),
                &sync_requested,
                &mut shutdown_rx,
            ),
        )
        .await
        .is_err(),
        "the retry wake was left queued for a duplicate sync"
    );
}

#[test]
fn requested_sync_retries_promptly_when_another_sync_is_busy() {
    let interval = Duration::from_secs(30);
    let request_pending = cloudsync_request_pending(false, CloudsyncWake::Requested);
    let request_pending = cloudsync_request_pending(request_pending, CloudsyncWake::Interval);

    assert_eq!(
        cloudsync_busy_delay(request_pending, interval),
        CLOUDSYNC_PROGRESS_INTERVAL,
        "a timer collision must preserve the pending requested sync"
    );
    assert_eq!(cloudsync_busy_delay(false, interval), interval);
}

#[tokio::test]
async fn after_sync_hook_receives_the_bounded_network_result() {
    let db = Db::connect_memory_plain().await.unwrap();
    let expected = CloudsyncNetworkResult {
        send: Some(anlg_cloudsync::NetworkSendResult {
            status: "synced".to_string(),
            local_version: 4,
            server_version: 4,
            chunks: 1,
            bytes: 1024,
            last_failure: None,
        }),
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 3,
            tables: vec!["sessions".to_string()],
            chunks: 1,
            bytes: 2048,
            complete: false,
            error: None,
            last_failure: None,
        }),
    };
    let recording_hook = Arc::new(RecordingSyncHook::default());
    let hook: Arc<dyn crate::CloudsyncSyncHook> = recording_hook.clone();
    let hook = Mutex::new(Some(hook));

    run_after_sync_hook(&hook, db.pool(), &expected)
        .await
        .unwrap();

    assert_eq!(
        recording_hook.after_result.lock().unwrap().as_ref(),
        Some(&expected)
    );
}

fn table_change(table: &str, seq: u64) -> anlg_db_change::TableChange {
    anlg_db_change::TableChange {
        table: table.to_string(),
        kind: anlg_db_change::TableChangeKind::Insert,
        seq,
    }
}

#[test]
fn change_wake_deadline_only_pulls_the_next_sync_earlier() {
    let base = tokio::time::Instant::now();
    let interval_deadline = base + Duration::from_secs(30);

    assert_eq!(
        cloudsync_wake_deadline(interval_deadline, None),
        interval_deadline
    );
    assert_eq!(
        cloudsync_wake_deadline(interval_deadline, Some(base + Duration::from_secs(1))),
        base + Duration::from_secs(1)
    );
    assert_eq!(
        cloudsync_wake_deadline(interval_deadline, Some(base + Duration::from_secs(60))),
        interval_deadline,
        "a change wake must never postpone the standing sync cadence"
    );
}

#[tokio::test]
async fn synced_table_changes_wake_and_unrelated_tables_do_not() {
    let (tx, mut rx) = tokio::sync::broadcast::channel(8);
    let synced = std::collections::HashSet::from(["sessions".to_string()]);
    let mut closed = false;

    tx.send(table_change("local_settings", 1)).unwrap();
    tx.send(table_change("sessions", 2)).unwrap();
    tokio::time::timeout(
        Duration::from_millis(100),
        next_synced_change(&mut rx, &synced, &mut closed),
    )
    .await
    .expect("a synced table change did not wake the loop");

    tx.send(table_change("local_settings", 3)).unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_millis(50),
            next_synced_change(&mut rx, &synced, &mut closed),
        )
        .await
        .is_err(),
        "an unrelated table change woke the sync loop"
    );
}

#[tokio::test]
async fn lagged_change_subscription_wakes_conservatively() {
    let (tx, mut rx) = tokio::sync::broadcast::channel(1);
    let synced = std::collections::HashSet::from(["sessions".to_string()]);
    let mut closed = false;

    tx.send(table_change("local_settings", 1)).unwrap();
    tx.send(table_change("local_settings", 2)).unwrap();
    tokio::time::timeout(
        Duration::from_millis(100),
        next_synced_change(&mut rx, &synced, &mut closed),
    )
    .await
    .expect("a lagged change subscription did not wake conservatively");
}

#[tokio::test]
async fn closed_change_subscription_never_wakes() {
    let (tx, mut rx) = tokio::sync::broadcast::channel::<anlg_db_change::TableChange>(1);
    let synced = std::collections::HashSet::from(["sessions".to_string()]);
    let mut closed = false;
    drop(tx);

    assert!(
        tokio::time::timeout(
            Duration::from_millis(50),
            next_synced_change(&mut rx, &synced, &mut closed),
        )
        .await
        .is_err(),
        "a closed change subscription completed a wake"
    );
    assert!(closed, "a closed change subscription was not flagged");
}

#[tokio::test]
async fn post_sync_drain_empties_the_queue_and_reports_synced_changes() {
    let (tx, mut rx) = tokio::sync::broadcast::channel(8);
    let synced = std::collections::HashSet::from(["sessions".to_string()]);
    let mut closed = false;

    tx.send(table_change("sessions", 1)).unwrap();
    tx.send(table_change("sessions", 2)).unwrap();
    assert!(
        drain_pending_changes(&mut rx, &synced),
        "a drained synced-table change did not request a follow-up round"
    );

    assert!(
        tokio::time::timeout(
            Duration::from_millis(50),
            next_synced_change(&mut rx, &synced, &mut closed),
        )
        .await
        .is_err(),
        "echoed changes survived the post-sync drain"
    );
}

#[test]
fn post_sync_drain_ignores_unrelated_changes() {
    let (tx, mut rx) = tokio::sync::broadcast::channel(8);
    let synced = std::collections::HashSet::from(["sessions".to_string()]);

    tx.send(table_change("local_settings", 1)).unwrap();
    assert!(
        !drain_pending_changes(&mut rx, &synced),
        "an unrelated drained change requested a follow-up round"
    );
    assert!(
        !drain_pending_changes(&mut rx, &synced),
        "an empty queue requested a follow-up round"
    );
}

#[test]
fn post_sync_drain_reports_lag_conservatively() {
    let (tx, mut rx) = tokio::sync::broadcast::channel(1);
    let synced = std::collections::HashSet::from(["sessions".to_string()]);

    tx.send(table_change("local_settings", 1)).unwrap();
    tx.send(table_change("local_settings", 2)).unwrap();
    assert!(
        drain_pending_changes(&mut rx, &synced),
        "a lagged drain did not request a follow-up round"
    );
}

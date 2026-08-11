use std::sync::Arc;
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::*;

/// Catches a deadlocked pool or worker, not slow I/O: a replacement pool connection
/// reloads and revalidates the sqlite-sync extension, which costs far more than a
/// query's worth of time on Windows CI.
const LIVENESS_TIMEOUT: Duration = Duration::from_secs(5);

async fn db_with_oversized_pending_payload() -> Db {
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
    sqlx::query("SELECT cloudsync_set('payload_max_chunk_size', '262144')")
        .fetch_optional(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO items (id, value) VALUES (?, ?)")
        .bind("large-value")
        .bind("x".repeat(3 * 1024 * 1024))
        .execute(db.pool())
        .await
        .unwrap();
    db
}

fn assert_outbound_payload_too_large(error: anlg_cloudsync::Error) {
    assert!(matches!(
        error,
        anlg_cloudsync::Error::OutboundPayloadTooLarge {
            chunks,
            max_chunks: CLOUDSYNC_MAX_OUTBOUND_CHUNKS,
            ..
        } if chunks == CLOUDSYNC_MAX_OUTBOUND_CHUNKS + 1
    ));
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[derive(Clone, Copy, Debug)]
enum StalledNetworkOperation {
    ManualSend,
    ManualReceive,
    LegacySync,
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
async fn assert_stalled_operation_drains_before_local_write(operation: StalledNetworkOperation) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (accepted_tx, accepted_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        accepted_tx.send(()).unwrap();
        let _stream = stream;
        let _ = release_rx.recv_timeout(Duration::from_secs(5));
    });

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
    if matches!(
        operation,
        StalledNetworkOperation::ManualSend | StalledNetworkOperation::LegacySync
    ) {
        sqlx::query("INSERT INTO items (id, value) VALUES ('before', 'pending')")
            .execute(db.pool())
            .await
            .unwrap();
    }
    {
        let mut connection = db.cloudsync_connection.lock().await;
        *connection = Some(db.pool.acquire().await.unwrap());
        sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
            .bind(endpoint)
            .bind("manual-interrupt-test")
            .fetch_optional(&mut **connection.as_mut().unwrap())
            .await
            .unwrap();
    }
    db.cloudsync_runtime.lock().unwrap().network_initialized = true;

    let cancelled = Arc::new(AtomicBool::new(false));
    let request_db = Arc::clone(&db);
    let request_cancelled = Arc::clone(&cancelled);
    let request = tokio::spawn(async move {
        match operation {
            StalledNetworkOperation::ManualSend => request_db
                .cloudsync_manual_send_only(request_cancelled.as_ref())
                .await
                .map(|_| ())
                .map_err(|error| error.to_string()),
            StalledNetworkOperation::ManualReceive => request_db
                .cloudsync_manual_receive_one()
                .await
                .map(|_| ())
                .map_err(|error| error.to_string()),
            StalledNetworkOperation::LegacySync => request_db
                .cloudsync_network_sync(None, None)
                .await
                .map(|_| ())
                .map_err(|error| error.to_string()),
        }
    });
    tokio::task::spawn_blocking(move || {
        accepted_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| {
                panic!("{operation:?} CloudSync request did not reach the blackhole server")
            });
    })
    .await
    .unwrap();

    cancelled.store(true, Ordering::Release);
    let interrupted_at = std::time::Instant::now();
    while !request.is_finished() {
        db.cloudsync_interrupt_sync();
        assert!(
            interrupted_at.elapsed() < Duration::from_secs(2),
            "manual CloudSync request did not honor sqlite3_interrupt"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(
        request.await.unwrap().is_err(),
        "interrupted manual request succeeded"
    );
    tokio::time::timeout(LIVENESS_TIMEOUT, db.cloudsync_wait_for_sync_idle())
        .await
        .expect("manual CloudSync worker did not become idle after interruption");
    db.cloudsync_connection.lock().await.take();
    tokio::time::timeout(
        LIVENESS_TIMEOUT,
        sqlx::query("INSERT INTO items (id, value) VALUES ('after', 'local')").execute(db.pool()),
    )
    .await
    .expect("immediate local write remained blocked after manual CloudSync interruption")
    .unwrap();

    let _ = release_tx.send(());
    server.join().unwrap();
    db.cloudsync_close_connection().await.unwrap();
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[tokio::test]
async fn manual_and_legacy_network_calls_drain_before_local_writes() {
    for _ in 0..3 {
        assert_stalled_operation_drains_before_local_write(StalledNetworkOperation::ManualSend)
            .await;
        assert_stalled_operation_drains_before_local_write(StalledNetworkOperation::ManualReceive)
            .await;
    }
    assert_stalled_operation_drains_before_local_write(StalledNetworkOperation::LegacySync).await;
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[tokio::test]
async fn native_logout_cleanup_interrupt_drains_before_local_write() {
    let directory = tempfile::tempdir().unwrap();
    let db = Arc::new(
        Db::connect_local(directory.path().join("logout-interrupt.db"))
            .await
            .unwrap(),
    );
    sqlx::query(
        "CREATE TABLE items (
            id TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
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
    db.cloudsync_init_enabled_tables(&[CloudsyncTableSpec {
        table_name: "items".to_string(),
        crdt_algo: None,
        init_flags: None,
        enabled: true,
    }])
    .await
    .unwrap();
    let request_db = Arc::clone(&db);
    let request = tokio::spawn(async move { request_db.cloudsync_network_logout().await });
    let interrupted_at = std::time::Instant::now();
    let mut registered = false;
    while !request.is_finished() {
        registered |= db.cloudsync_interrupt_sync();
        assert!(
            interrupted_at.elapsed() < Duration::from_secs(2),
            "native CloudSync logout did not honor sqlite3_interrupt"
        );
        tokio::task::yield_now().await;
    }
    assert!(
        registered,
        "native CloudSync logout completed before its interrupt registration was observable"
    );
    assert!(
        request.await.unwrap().is_err(),
        "interrupted native CloudSync logout succeeded"
    );
    tokio::time::timeout(LIVENESS_TIMEOUT, db.cloudsync_wait_for_sync_idle())
        .await
        .expect("native CloudSync logout worker did not become idle after interruption");
    db.cloudsync_version()
        .await
        .expect("native CloudSync logout interruption crashed the pinned SQLite worker");
    db.cloudsync_terminate_and_close().await.unwrap();
    tokio::time::timeout(
        LIVENESS_TIMEOUT,
        sqlx::query("INSERT INTO items (id, value) VALUES ('after', 'local')").execute(db.pool()),
    )
    .await
    .expect("immediate local write remained blocked after native CloudSync logout interruption")
    .unwrap();
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[tokio::test]
async fn tracked_table_cleanup_interrupt_drains_before_local_write() {
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
    let tracking_schema_before: Vec<(String, String)> = sqlx::query_as(
        "SELECT type, name
         FROM sqlite_schema
         WHERE (type = 'table' AND name = 'items_cloudsync')
            OR (type = 'trigger' AND tbl_name = 'items')
         ORDER BY type, name",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();

    let request_db = Arc::clone(&db);
    let request = tokio::spawn(async move { request_db.cloudsync_cleanup("items").await });
    let interrupted_at = std::time::Instant::now();
    let mut registered = false;
    while !request.is_finished() {
        registered |= db.cloudsync_interrupt_sync();
        assert!(
            interrupted_at.elapsed() < Duration::from_secs(2),
            "native CloudSync table cleanup did not honor sqlite3_interrupt"
        );
        tokio::task::yield_now().await;
    }
    assert!(
        registered,
        "native CloudSync table cleanup completed before its interrupt registration was observable"
    );
    assert!(
        request.await.unwrap().is_err(),
        "interrupted native CloudSync table cleanup succeeded"
    );
    tokio::time::timeout(LIVENESS_TIMEOUT, db.cloudsync_wait_for_sync_idle())
        .await
        .expect("native CloudSync table cleanup worker did not become idle after interruption");
    db.cloudsync_version()
        .await
        .expect("native CloudSync table cleanup interruption crashed the pinned SQLite worker");
    let tracking_schema_after: Vec<(String, String)> = sqlx::query_as(
        "SELECT type, name
         FROM sqlite_schema
         WHERE (type = 'table' AND name = 'items_cloudsync')
            OR (type = 'trigger' AND tbl_name = 'items')
         ORDER BY type, name",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        tracking_schema_after, tracking_schema_before,
        "interrupted native CloudSync table cleanup left partial tracking schema"
    );
    tokio::time::timeout(
        LIVENESS_TIMEOUT,
        sqlx::query("INSERT INTO items (id, value) VALUES ('after', 'local')").execute(db.pool()),
    )
    .await
    .expect(
        "immediate local write remained blocked after native CloudSync table cleanup interruption",
    )
    .unwrap();
    db.cloudsync_cleanup("items")
        .await
        .expect("CloudSync table cleanup retry failed after interruption");
    let tracking_schema_after_retry: Vec<(String, String)> = sqlx::query_as(
        "SELECT type, name
         FROM sqlite_schema
         WHERE (type = 'table' AND name = 'items_cloudsync')
            OR (type = 'trigger' AND tbl_name = 'items')
         ORDER BY type, name",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert!(
        tracking_schema_after_retry.is_empty(),
        "successful CloudSync table cleanup retry left tracking schema"
    );
    sqlx::query("INSERT INTO items (id, value) VALUES ('after-retry', 'local')")
        .execute(db.pool())
        .await
        .expect("local write failed after successful CloudSync table cleanup retry");
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[tokio::test]
async fn populated_table_init_interrupt_drains_before_local_write() {
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

    let request_db = Arc::clone(&db);
    let request = tokio::spawn(async move {
        request_db
            .cloudsync_init_enabled_tables(&[CloudsyncTableSpec {
                table_name: "items".to_string(),
                crdt_algo: None,
                init_flags: None,
                enabled: true,
            }])
            .await
    });
    let interrupted_at = std::time::Instant::now();
    let mut registered = false;
    while !request.is_finished() {
        registered |= db.cloudsync_interrupt_sync();
        assert!(
            interrupted_at.elapsed() < Duration::from_secs(2),
            "native CloudSync table initialization did not honor sqlite3_interrupt"
        );
        tokio::task::yield_now().await;
    }
    assert!(
        registered,
        "native CloudSync table initialization completed before its interrupt registration was observable"
    );
    assert!(
        request.await.unwrap().is_err(),
        "interrupted native CloudSync table initialization succeeded"
    );
    tokio::time::timeout(LIVENESS_TIMEOUT, db.cloudsync_wait_for_sync_idle())
        .await
        .expect(
            "native CloudSync table initialization worker did not become idle after interruption",
        );
    db.cloudsync_version().await.expect(
        "native CloudSync table initialization interruption crashed the pinned SQLite worker",
    );
    tokio::time::timeout(
        LIVENESS_TIMEOUT,
        sqlx::query("INSERT INTO items (id, value) VALUES ('after', 'local')")
            .execute(db.pool()),
    )
    .await
    .expect(
        "immediate local write remained blocked after native CloudSync table initialization interruption",
    )
    .unwrap();
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[tokio::test]
async fn pending_payload_interrupt_drains_before_local_write() {
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

    let request_db = Arc::clone(&db);
    let request = tokio::spawn(async move { request_db.cloudsync_pending_payload_batch().await });
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
    let interrupted_at = std::time::Instant::now();
    while !request.is_finished() {
        assert!(db.cloudsync_interrupt_sync());
        assert!(
            interrupted_at.elapsed() < Duration::from_secs(2),
            "pending payload generation did not honor sqlite3_interrupt"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(
        request.await.unwrap().is_err(),
        "interrupted pending payload generation succeeded"
    );
    tokio::time::timeout(LIVENESS_TIMEOUT, db.cloudsync_wait_for_sync_idle())
        .await
        .expect("pending payload worker did not become idle after interruption");
    db.cloudsync_connection.lock().await.take();
    tokio::time::timeout(
        LIVENESS_TIMEOUT,
        sqlx::query("INSERT INTO items (id, value) VALUES ('after', 'local')").execute(db.pool()),
    )
    .await
    .expect("immediate local write remained blocked after pending payload interruption")
    .unwrap();
}

#[test]
fn reconciled_send_reports_the_exact_preflighted_batch() {
    let batch = anlg_cloudsync::PendingPayloadBatch {
        start_db_version: 4,
        watermark_db_version: Some(9),
        chunks: 2,
        rows: 8,
        bytes: 4096,
        complete: true,
        fits: true,
    };
    let status = anlg_cloudsync::NetworkStatus {
        last_optimistic_version: 12,
        last_confirmed_version: 12,
        gaps: Vec::new(),
        failures: anlg_cloudsync::NetworkStatusFailures::default(),
    };

    let result = reconciled_send_result(batch, &status);
    let send = result.send.unwrap();

    assert_eq!(send.status, "synced");
    assert_eq!(send.local_version, 9);
    assert_eq!(send.server_version, 12);
    assert_eq!(send.chunks, 2);
    assert_eq!(send.bytes, 4096);
}

#[test]
fn cancelled_send_never_starts_status_reconciliation() {
    let batch = anlg_cloudsync::PendingPayloadBatch {
        start_db_version: 4,
        watermark_db_version: Some(9),
        chunks: 2,
        rows: 8,
        bytes: 4096,
        complete: true,
        fits: true,
    };
    let error = anlg_cloudsync::Error::Io(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "interrupted upload",
    ));

    assert!(should_reconcile_send_failure(batch, &error, false));
    assert!(!should_reconcile_send_failure(batch, &error, true));
}

#[test]
fn bounded_sync_preserves_the_send_and_single_receive_results() {
    let send = anlg_cloudsync::NetworkResult {
        send: Some(anlg_cloudsync::NetworkSendResult {
            status: "synced".to_string(),
            local_version: 9,
            server_version: 9,
            chunks: 2,
            bytes: 4096,
            last_failure: None,
        }),
        receive: None,
    };
    let receive = anlg_cloudsync::NetworkResult {
        send: None,
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 3,
            tables: vec!["items".to_string()],
            chunks: 1,
            bytes: 2048,
            complete: false,
            error: None,
            last_failure: None,
        }),
    };

    let result = merge_bounded_sync_results(send, receive);

    assert_eq!(result.send.unwrap().chunks, 2);
    let receive = result.receive.unwrap();
    assert_eq!(receive.chunks, 1);
    assert!(!receive.complete);
}

#[tokio::test]
async fn oversized_payload_never_reaches_send_transport() {
    let db = db_with_oversized_pending_payload().await;

    let error = db.cloudsync_network_send_changes().await.unwrap_err();

    assert_outbound_payload_too_large(error);
}

#[tokio::test]
async fn public_network_sync_cannot_bypass_the_send_guard() {
    let db = db_with_oversized_pending_payload().await;

    let error = db.cloudsync_network_sync(None, None).await.unwrap_err();

    assert_outbound_payload_too_large(error);
}

#[tokio::test]
async fn network_calls_reuse_one_checked_out_connection() {
    let db = Arc::new(Db::connect_memory_plain().await.unwrap());
    {
        let mut connection = db.lock_cloudsync_connection().await.unwrap();
        sqlx::query("CREATE TEMP TABLE cloudsync_connection_marker (value INTEGER)")
            .execute(&mut **connection.as_mut().unwrap())
            .await
            .unwrap();
    }

    let mut connection = db.lock_cloudsync_connection().await.unwrap();
    let marker_exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_temp_master WHERE name = 'cloudsync_connection_marker'",
    )
    .fetch_one(&mut **connection.as_mut().unwrap())
    .await
    .unwrap();

    assert_eq!(marker_exists, 1);
}

#[tokio::test]
async fn network_sync_waits_for_checked_out_connection() {
    let db = Arc::new(Db::connect_memory_plain().await.unwrap());
    let guard = db.lock_cloudsync_connection().await.unwrap();
    let task_db = Arc::clone(&db);
    let mut task = tokio::spawn(async move { task_db.cloudsync_network_sync(None, None).await });

    assert!(
        tokio::time::timeout(Duration::from_millis(25), &mut task)
            .await
            .is_err()
    );

    drop(guard);
    assert!(
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn initializing_tables_updates_every_pool_connection() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cloudsync.db");
    let db = Db::open(crate::DbOpenOptions {
        storage: crate::DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE sessions (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let mut preexisting_connections = Vec::new();
    for _ in 0..4 {
        preexisting_connections.push(db.pool().acquire().await.unwrap());
    }
    drop(preexisting_connections);

    db.cloudsync_init_enabled_tables(&[CloudsyncTableSpec {
        table_name: "sessions".to_string(),
        crdt_algo: None,
        init_flags: None,
        enabled: true,
    }])
    .await
    .unwrap();

    let mut write_connections = Vec::new();
    for _ in 0..3 {
        write_connections.push(db.pool().acquire().await.unwrap());
    }
    for (index, connection) in write_connections.iter_mut().enumerate() {
        sqlx::query("INSERT INTO sessions (id, title) VALUES (?, 'Note')")
            .bind(format!("session-{index}"))
            .execute(&mut **connection)
            .await
            .unwrap();
    }
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn table_init_bounds_pool_drain_and_reuses_connections_after_retry() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cloudsync.db");
    let db = Db::open(crate::DbOpenOptions {
        storage: crate::DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(3),
    })
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE sessions (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let tables = [CloudsyncTableSpec {
        table_name: "sessions".to_string(),
        crdt_algo: None,
        init_flags: None,
        enabled: true,
    }];
    let mut held_connection = db.pool().acquire().await.unwrap();

    let started_at = std::time::Instant::now();
    let error = tokio::time::timeout(
        Duration::from_millis(1_500),
        db.cloudsync_init_enabled_tables(&tables),
    )
    .await
    .expect("CloudSync table initialization exceeded its pool-drain deadline")
    .unwrap_err();
    assert!(matches!(error, CloudsyncRuntimeError::LocalStatusBusy));
    assert!(started_at.elapsed() < Duration::from_millis(1_500));

    held_connection.return_to_pool().await;
    db.cloudsync_init_enabled_tables(&tables).await.unwrap();

    let mut connections = Vec::new();
    for index in 0..2 {
        let mut connection = tokio::time::timeout(LIVENESS_TIMEOUT, db.pool().acquire())
            .await
            .expect("CloudSync table initialization leaked a pool connection")
            .unwrap();
        assert!(
            cloudsync_is_enabled_on(&mut *connection, "sessions")
                .await
                .unwrap()
        );
        sqlx::query("INSERT INTO sessions (id, title) VALUES (?, 'Note')")
            .bind(format!("session-{index}"))
            .execute(&mut *connection)
            .await
            .unwrap();
        connections.push(connection);
    }
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn table_init_error_closes_partially_initialized_drained_connections() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cloudsync.db");
    let db = Db::open(crate::DbOpenOptions {
        storage: crate::DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(3),
    })
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE sessions (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let mut preexisting_connections = Vec::new();
    for _ in 0..3 {
        let mut connection = db.pool().acquire().await.unwrap();
        sqlx::query("CREATE TEMP TABLE init_connection_marker (id INTEGER)")
            .execute(&mut *connection)
            .await
            .unwrap();
        preexisting_connections.push(connection);
    }
    for connection in &mut preexisting_connections {
        connection.return_to_pool().await;
    }

    db.cloudsync_init_enabled_tables(&[
        CloudsyncTableSpec {
            table_name: "sessions".to_string(),
            crdt_algo: None,
            init_flags: None,
            enabled: true,
        },
        CloudsyncTableSpec {
            table_name: "missing_table".to_string(),
            crdt_algo: None,
            init_flags: None,
            enabled: true,
        },
    ])
    .await
    .unwrap_err();

    let mut replacements = Vec::new();
    for _ in 0..2 {
        let mut connection = tokio::time::timeout(LIVENESS_TIMEOUT, db.pool().acquire())
            .await
            .expect("failed initialization leaked a drained pool connection")
            .unwrap();
        let marker_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM sqlite_temp_master
             WHERE name = 'init_connection_marker'",
        )
        .fetch_one(&mut *connection)
        .await
        .unwrap();
        assert_eq!(marker_count, 0);
        replacements.push(connection);
    }
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn receive_version_reset_preserves_send_cursor_on_pinned_connection() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cloudsync.db");
    let db = Db::open(crate::DbOpenOptions {
        storage: crate::DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(2),
    })
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE sessions (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db.cloudsync_init("sessions", None, None).await.unwrap();

    {
        let mut connection = db.lock_cloudsync_connection().await.unwrap();
        sqlx::query("INSERT INTO sessions (id, title) VALUES ('local', 'Pending')")
            .execute(&mut **connection.as_mut().unwrap())
            .await
            .unwrap();
        sqlx::query(
            "SELECT
                cloudsync_set('check_dbversion', '31'),
                cloudsync_set('check_seq', '7'),
                cloudsync_set('send_dbversion', '19'),
                cloudsync_set('send_seq', '4')",
        )
        .fetch_optional(&mut **connection.as_mut().unwrap())
        .await
        .unwrap();
    }

    db.cloudsync_network_reset_receive_version().await.unwrap();

    let mut connection = db.lock_cloudsync_connection().await.unwrap();
    let cursors: (String, String, String, String) = sqlx::query_as(
        "SELECT
            MAX(CASE WHEN key = 'check_dbversion' THEN value END),
            MAX(CASE WHEN key = 'check_seq' THEN value END),
            MAX(CASE WHEN key = 'send_dbversion' THEN value END),
            MAX(CASE WHEN key = 'send_seq' THEN value END)
         FROM cloudsync_settings",
    )
    .fetch_one(&mut **connection.as_mut().unwrap())
    .await
    .unwrap();
    assert_eq!(cursors, ("0".into(), "0".into(), "19".into(), "4".into()));
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn terminating_cloudsync_closes_a_single_pool_connection() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cloudsync.db");
    let db = Db::open(crate::DbOpenOptions {
        storage: crate::DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE sessions (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db.cloudsync_init("sessions", None, None).await.unwrap();
    {
        let mut connection = db.lock_cloudsync_connection().await.unwrap();
        sqlx::query("CREATE TEMP TABLE connection_marker (id INTEGER)")
            .execute(&mut **connection.as_mut().unwrap())
            .await
            .unwrap();
    }

    db.cloudsync_terminate_and_close().await.unwrap();

    let marker_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_temp_master WHERE name = 'connection_marker'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(marker_count, 0);
    sqlx::query("INSERT INTO sessions (id, title) VALUES ('session', 'Note')")
        .execute(db.pool())
        .await
        .unwrap();
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn terminating_cloudsync_closes_every_pool_connection() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cloudsync.db");
    let db = Db::open(crate::DbOpenOptions {
        storage: crate::DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(3),
    })
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE sessions (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db.cloudsync_init_enabled_tables(&[CloudsyncTableSpec {
        table_name: "sessions".to_string(),
        crdt_algo: None,
        init_flags: None,
        enabled: true,
    }])
    .await
    .unwrap();

    let mut connections = Vec::new();
    for _ in 0..2 {
        let mut connection = db.pool().acquire().await.unwrap();
        sqlx::query("CREATE TEMP TABLE connection_marker (id INTEGER)")
            .execute(&mut *connection)
            .await
            .unwrap();
        connections.push(connection);
    }
    drop(connections);

    db.cloudsync_terminate_and_close().await.unwrap();

    let mut replacements = Vec::new();
    for index in 0..3 {
        let mut connection = db.pool().acquire().await.unwrap();
        let marker_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_temp_master WHERE name = 'connection_marker'",
        )
        .fetch_one(&mut *connection)
        .await
        .unwrap();
        assert_eq!(marker_count, 0);
        sqlx::query("INSERT INTO sessions (id, title) VALUES (?, 'Note')")
            .bind(format!("session-{index}"))
            .execute(&mut *connection)
            .await
            .unwrap();
        replacements.push(connection);
    }
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn suspend_bounds_pool_drain_and_allows_later_teardown() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cloudsync.db");
    let db = Db::open(crate::DbOpenOptions {
        storage: crate::DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(3),
    })
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE sessions (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db.cloudsync_init_enabled_tables(&[CloudsyncTableSpec {
        table_name: "sessions".to_string(),
        crdt_algo: None,
        init_flags: None,
        enabled: true,
    }])
    .await
    .unwrap();
    let mut held_connection = db.pool().acquire().await.unwrap();

    let started_at = std::time::Instant::now();
    let error = tokio::time::timeout(Duration::from_millis(1_500), db.cloudsync_suspend())
        .await
        .expect("CloudSync suspend exceeded its pool-drain deadline")
        .unwrap_err();
    assert!(matches!(error, CloudsyncRuntimeError::LocalStatusBusy));
    assert!(started_at.elapsed() < Duration::from_millis(1_500));

    held_connection.return_to_pool().await;
    tokio::time::timeout(
        Duration::from_millis(1_500),
        sqlx::query("INSERT INTO sessions (id, title) VALUES ('after-timeout', 'Note')")
            .execute(db.pool()),
    )
    .await
    .expect("local write could not reuse the pool after the drain timeout")
    .unwrap();

    db.cloudsync_terminate_and_close().await.unwrap();
    tokio::time::timeout(
        LIVENESS_TIMEOUT,
        sqlx::query("INSERT INTO sessions (id, title) VALUES ('after-retry', 'Note')")
            .execute(db.pool()),
    )
    .await
    .expect("local write could not reuse the pool after teardown retry")
    .unwrap();
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn suspend_bounds_initial_pinned_connection_acquisition() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cloudsync.db");
    let db = Db::open(crate::DbOpenOptions {
        storage: crate::DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE sessions (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let mut held_connection = db.pool().acquire().await.unwrap();

    let error = tokio::time::timeout(Duration::from_millis(1_500), db.cloudsync_suspend())
        .await
        .expect("CloudSync suspend exceeded its initial pool-acquisition deadline")
        .unwrap_err();
    assert!(matches!(error, CloudsyncRuntimeError::LocalStatusBusy));

    held_connection.return_to_pool().await;
    db.cloudsync_suspend().await.unwrap();
    tokio::time::timeout(
        LIVENESS_TIMEOUT,
        sqlx::query("INSERT INTO sessions (id, title) VALUES ('after-retry', 'Note')")
            .execute(db.pool()),
    )
    .await
    .expect("local write could not reuse the pool after initial acquisition retry")
    .unwrap();
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn initializes_replacement_pool_connections() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cloudsync.db");
    let db = Db::open(crate::DbOpenOptions {
        storage: crate::DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(2),
    })
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE sessions (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    db.cloudsync_init_enabled_tables(&[CloudsyncTableSpec {
        table_name: "sessions".to_string(),
        crdt_algo: None,
        init_flags: None,
        enabled: true,
    }])
    .await
    .unwrap();

    let connection = db.pool().acquire().await.unwrap();
    connection.close().await.unwrap();
    let mut replacement = db.pool().acquire().await.unwrap();

    sqlx::query("INSERT INTO sessions (id, title) VALUES ('replacement', 'Note')")
        .execute(&mut *replacement)
        .await
        .unwrap();
}

#[tokio::test]
async fn closes_pinned_connection_without_cloudsync_extension() {
    let db = Db::connect_local_plain(tempfile::NamedTempFile::new().unwrap().path())
        .await
        .unwrap();
    drop(db.lock_cloudsync_connection().await.unwrap());
    assert!(db.cloudsync_connection.lock().await.is_some());

    db.cloudsync_close_connection().await.unwrap();

    assert!(db.cloudsync_connection.lock().await.is_none());
}

#![deny(unsafe_code)]

mod api;
mod bundle;
mod close;
mod error;
mod locked;
mod network;

use std::path::PathBuf;

use sqlx::sqlite::SqliteConnectOptions;

pub use api::{
    CloudsyncConnectionInitializer, CloudsyncTableSpec, begin_alter, cleanup,
    cleanup_on_connection, commit_alter, db_version, disable, enable, init, init_on_connection,
    is_enabled, set_filter, siteid, terminate, uuid, version,
};
pub use bundle::bundled_extension_path;
pub use close::install_transaction_observer;
pub use error::{Error, ErrorKind};
pub use locked::{OwnedSqliteConnection, ReservedConnection};
pub use network::{
    CLOUDSYNC_NETWORK_CONNECT_TIMEOUT_SECONDS, CLOUDSYNC_NETWORK_REQUEST_TIMEOUT_SECONDS,
    NetworkReceiveResult, NetworkResult, NetworkSendResult, NetworkStatus, NetworkStatusFailures,
    PendingPayloadBatch, network_check_changes, network_cleanup, network_has_unsent_changes,
    network_init, network_logout, network_logout_on_connection, network_receive_changes,
    network_receive_changes_on_connection, network_reset_receive_version,
    network_reset_sync_version, network_send_changes, network_send_changes_bounded,
    network_send_changes_bounded_on_connection, network_set_apikey, network_set_request_deadlines,
    network_set_token, network_status, network_status_on_connection, network_sync,
    pending_payload_batch, reconcile_confirmed_pending_payload,
};

pub const CLOUDSYNC_VERSION: &str = "1.2.0";

pub fn apply(options: SqliteConnectOptions) -> Result<(SqliteConnectOptions, PathBuf), Error> {
    close::install_terminate_on_close()?;
    let extension_path = bundled_extension_path()?;

    #[allow(unsafe_code)]
    let options = unsafe { options.extension(extension_path.to_string_lossy().into_owned()) };

    Ok((options, extension_path))
}

pub fn apply_with_initializer(
    options: SqliteConnectOptions,
    initializer: &CloudsyncConnectionInitializer,
) -> Result<(SqliteConnectOptions, PathBuf), Error> {
    close::install_terminate_on_close()?;
    let extension_path = bundled_extension_path()?;
    initializer.set_extension_path(extension_path.clone(), options.get_filename().to_path_buf());
    Ok((options, extension_path))
}

#[cfg(any(
    all(test, target_os = "ios"),
    all(test, target_os = "macos", target_arch = "aarch64"),
    all(test, target_os = "macos", target_arch = "x86_64"),
    all(test, target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(test, target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(
        test,
        target_os = "linux",
        target_env = "musl",
        target_arch = "aarch64"
    ),
    all(test, target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(test, target_os = "windows", target_arch = "x86_64"),
))]
mod tests {
    use super::*;
    use std::future::Future;
    use std::str::FromStr;
    use std::time::{Duration, Instant};

    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn loads_bundled_cloudsync() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();

        let version = version(&pool).await.unwrap();

        assert_eq!(version, CLOUDSYNC_VERSION);
        pool.close().await;
    }

    #[tokio::test]
    async fn sqlite_misuse_does_not_kill_the_sqlx_connection_worker() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let connection = pool.acquire().await.unwrap();
        let mut connection = ReservedConnection::new(connection);

        let error = locked::execute_on_locked_handle(
            &mut connection,
            "SELECT cloudsync_payload_decode('x')",
            Vec::new(),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(&error, Error::Sqlite { code: 21, .. }),
            "unexpected error: {error}"
        );

        let result: i64 = sqlx::query_scalar("SELECT 1")
            .fetch_one(connection.connection().await.unwrap())
            .await
            .unwrap();
        assert_eq!(result, 1);

        drop(connection.into_inner().unwrap());
        pool.close().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancelled_raw_sqlite_keeps_the_connection_reserved_without_blocking_the_runtime() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let mut connection = ReservedConnection::new(pool.acquire().await.unwrap());
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        register_test_gate(
            connection.connection().await.unwrap(),
            entered_tx,
            release_rx,
        )
        .await;

        let mut operation = Box::pin(locked::execute_on_locked_handle(
            &mut connection,
            "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE test_gate(x)) SELECT count(*) FROM c",
            vec![],
        ));
        let mut entered = tokio::task::spawn_blocking(move || {
            entered_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("SQLite did not enter the test gate");
        });
        tokio::select! {
            biased;
            result = &mut entered => result.unwrap(),
            result = operation.as_mut() => panic!("raw SQLite query completed before cancellation: {result:?}"),
        }

        let releaser = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            release_tx.send(()).unwrap();
        });
        let started = Instant::now();
        drop(operation);
        let cancelled_at = Instant::now();
        let drop_elapsed = started.elapsed();
        assert!(
            drop_elapsed < Duration::from_millis(100),
            "cancelling the SQLite query blocked for {drop_elapsed:?}"
        );

        tokio::time::timeout(
            Duration::from_millis(100),
            tokio::time::sleep(Duration::from_millis(10)),
        )
        .await
        .expect("current-thread runtime stopped making progress");
        let (responder_tx, responder_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _ = responder_tx.send(());
        });
        tokio::time::timeout(Duration::from_millis(100), responder_rx)
            .await
            .expect("responder task did not run")
            .unwrap();
        assert!(
            pool.try_acquire().is_none(),
            "cancelled SQLite connection was returned while its worker was active"
        );

        let query_result: i64 = sqlx::query_scalar("SELECT 1")
            .fetch_one(connection.connection().await.unwrap())
            .await
            .unwrap();
        assert_eq!(query_result, 1);
        assert!(
            cancelled_at.elapsed() < Duration::from_secs(2),
            "reclaiming the cancelled SQLite connection took {:?}",
            cancelled_at.elapsed()
        );
        releaser.join().unwrap();
        drop(connection);
        let connection = pool.acquire().await.unwrap();
        drop(connection);
        pool.close().await;
    }

    #[allow(unsafe_code)]
    async fn register_test_gate(
        connection: &mut sqlx::SqliteConnection,
        entered: std::sync::mpsc::Sender<()>,
        release: std::sync::mpsc::Receiver<()>,
    ) {
        use libsqlite3_sys::{
            SQLITE_UTF8, sqlite3_context, sqlite3_create_function_v2, sqlite3_result_int,
            sqlite3_user_data, sqlite3_value,
        };

        struct Gate {
            entered: std::sync::mpsc::Sender<()>,
            release: std::sync::mpsc::Receiver<()>,
            first_call: bool,
        }

        unsafe extern "C" fn gate(
            context: *mut sqlite3_context,
            _argument_count: i32,
            _arguments: *mut *mut sqlite3_value,
        ) {
            let gate = unsafe { &mut *sqlite3_user_data(context).cast::<Gate>() };
            if gate.first_call {
                gate.first_call = false;
                let _ = gate.entered.send(());
                let _ = gate.release.recv();
            }
            unsafe { sqlite3_result_int(context, 1) };
        }

        unsafe extern "C" fn destroy(app: *mut std::ffi::c_void) {
            drop(unsafe { Box::from_raw(app.cast::<Gate>()) });
        }

        let mut handle = connection.lock_handle().await.unwrap();
        let app = Box::into_raw(Box::new(Gate {
            entered,
            release,
            first_call: true,
        }));
        let result = unsafe {
            sqlite3_create_function_v2(
                handle.as_raw_handle().as_ptr(),
                c"test_gate".as_ptr(),
                1,
                SQLITE_UTF8,
                app.cast(),
                Some(gate),
                None,
                None,
                Some(destroy),
            )
        };
        assert_eq!(result, libsqlite3_sys::SQLITE_OK);
    }

    #[test]
    fn cancelling_raw_sqlite_before_worker_start_interrupts_the_worker() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(1)
            .enable_time()
            .build()
            .unwrap();

        runtime.block_on(async {
            let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
            let (options, _) = apply(options).unwrap();
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .unwrap();
            let mut connection = ReservedConnection::new(pool.acquire().await.unwrap());
            let result: i64 = sqlx::query_scalar("SELECT 1")
                .fetch_one(connection.connection().await.unwrap())
                .await
                .unwrap();
            assert_eq!(result, 1);

            let (started_tx, started_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                let _ = started_tx.send(());
                release_rx.recv().unwrap();
            });
            started_rx.await.unwrap();

            let mut operation = Box::pin(locked::execute_on_locked_handle(
                &mut connection,
                "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) SELECT count(*) FROM c",
                vec![],
            ));
            for _ in 0..4 {
                futures_util::future::poll_fn(|cx| match operation.as_mut().poll(cx) {
                    std::task::Poll::Pending => std::task::Poll::Ready(()),
                    std::task::Poll::Ready(result) => {
                        panic!("raw SQLite query completed before cancellation: {result:?}")
                    }
                })
                .await;
                tokio::task::yield_now().await;
            }

            let releaser = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(500));
                release_tx.send(()).unwrap();
            });
            let started = Instant::now();
            drop(operation);
            let elapsed = started.elapsed();
            assert!(
                elapsed < Duration::from_millis(100),
                "pre-step cancellation blocked for {elapsed:?}"
            );
            assert!(
                pool.try_acquire().is_none(),
                "cancelled SQLite connection was returned before its worker started"
            );

            drop(connection);
            let mut connection = pool.acquire().await.unwrap();
            let result: i64 = sqlx::query_scalar("SELECT 1")
                .fetch_one(&mut *connection)
                .await
                .unwrap();
            assert_eq!(result, 1);

            blocker.await.unwrap();
            releaser.join().unwrap();
            drop(connection);
            pool.close().await;
        });
    }

    #[test]
    fn cancelling_raw_sqlite_before_owner_starts_skips_the_worker() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(1)
            .enable_time()
            .build()
            .unwrap();

        runtime.block_on(async {
            let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
            let (options, _) = apply(options).unwrap();
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .unwrap();
            let mut connection = ReservedConnection::new(pool.acquire().await.unwrap());
            let result: i64 = sqlx::query_scalar("SELECT 1")
                .fetch_one(connection.connection().await.unwrap())
                .await
                .unwrap();
            assert_eq!(result, 1);

            let (started_tx, started_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                let _ = started_tx.send(());
                release_rx.recv().unwrap();
            });
            started_rx.await.unwrap();

            let mut operation = Box::pin(locked::execute_on_locked_handle(
                &mut connection,
                "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) SELECT count(*) FROM c",
                vec![],
            ));
            futures_util::future::poll_fn(|cx| match operation.as_mut().poll(cx) {
                std::task::Poll::Pending => std::task::Poll::Ready(()),
                std::task::Poll::Ready(result) => {
                    panic!("raw SQLite query completed before cancellation: {result:?}")
                }
            })
            .await;

            let releaser = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(500));
                release_tx.send(()).unwrap();
            });
            let started = Instant::now();
            drop(operation);
            let elapsed = started.elapsed();
            assert!(
                elapsed < Duration::from_millis(100),
                "pre-step cancellation blocked for {elapsed:?}"
            );
            assert!(
                pool.try_acquire().is_none(),
                "cancelled SQLite connection was returned before its worker started"
            );

            let result: i64 = {
                let connection = tokio::time::timeout(
                    Duration::from_millis(100),
                    connection.connection(),
                )
                .await
                .expect("cancelled owner waited for an unstarted native worker")
                .unwrap();
                sqlx::query_scalar("SELECT 1")
                    .fetch_one(connection)
                    .await
                    .unwrap()
            };
            assert_eq!(result, 1);

            drop(connection.into_inner().unwrap());
            let mut connection = pool.acquire().await.unwrap();
            let result: i64 = sqlx::query_scalar("SELECT 1")
                .fetch_one(&mut *connection)
                .await
                .unwrap();
            assert_eq!(result, 1);

            blocker.await.unwrap();
            releaser.join().unwrap();
            drop(connection);
            pool.close().await;
        });
    }

    #[tokio::test]
    async fn chunks_large_values_within_the_transport_limit() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        init(&pool, "items", None, None).await.unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES (?, ?)")
            .bind("large-value")
            .bind("x".repeat(12 * 1024 * 1024))
            .execute(&pool)
            .await
            .unwrap();

        let (chunks, total_bytes, max_chunk_bytes): (i64, i64, i64) = sqlx::query_as(
            "SELECT count(*), sum(payload_size), max(payload_size)
             FROM cloudsync_payload_chunks",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(chunks >= 3);
        assert!(total_bytes > 0);
        assert!(max_chunk_bytes <= 5 * 1024 * 1024);
        pool.close().await;
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "linux",
        target_os = "windows",
        target_os = "ios"
    ))]
    #[tokio::test]
    async fn native_http_request_deadline_is_enforced() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let _stream = stream;
            std::thread::sleep(std::time::Duration::from_secs(60));
        });

        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        init(&pool, "items", None, None).await.unwrap();
        sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
            .bind(address)
            .bind("deadline-test")
            .fetch_optional(&pool)
            .await
            .unwrap();
        sqlx::query("SELECT cloudsync_set('network_connect_timeout', '1')")
            .fetch_optional(&pool)
            .await
            .unwrap();
        sqlx::query("SELECT cloudsync_set('network_request_timeout', '1')")
            .fetch_optional(&pool)
            .await
            .unwrap();

        let started = std::time::Instant::now();
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            network_receive_changes(&pool, Some(1)),
        )
        .await
        .expect("native CloudSync request exceeded the timeout watchdog")
        .unwrap_err();
        let elapsed = started.elapsed();

        assert!(
            elapsed < std::time::Duration::from_secs(3),
            "native request took {elapsed:?}"
        );
        assert_eq!(error.kind(), ErrorKind::Transient, "{error}");

        let cleanup_started = std::time::Instant::now();
        network_cleanup(&pool).await.unwrap();
        pool.close().await;
        assert!(
            cleanup_started.elapsed() < std::time::Duration::from_secs(2),
            "native cleanup remained blocked after the request deadline"
        );
    }

    #[tokio::test]
    async fn large_pending_backlog_drains_in_complete_version_windows() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let sender = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let receiver_options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (receiver_options, _) = apply(receiver_options).unwrap();
        let receiver = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(receiver_options)
            .await
            .unwrap();
        for pool in [&sender, &receiver] {
            sqlx::query(
                "CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL DEFAULT '')",
            )
            .execute(pool)
            .await
            .unwrap();
            init(pool, "items", None, None).await.unwrap();
        }
        let mut connection = sender.acquire().await.unwrap();
        for index in 0..240 {
            sqlx::query("INSERT INTO items (id, value) VALUES (?, ?)")
                .bind(index.to_string())
                .bind(format!("value-{index}"))
                .execute(&mut *connection)
                .await
                .unwrap();
        }

        let total_rows: i64 = sqlx::query_scalar("SELECT SUM(rows) FROM cloudsync_payload_chunks")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        assert!(
            total_rows > 32,
            "the old whole-backlog preflight must reject this fixture"
        );
        let mut batches = 0;
        let mut sent_local_versions = 0;
        loop {
            let batch = pending_payload_batch(&mut connection, 8, 32, 32 * 1024 * 1024)
                .await
                .unwrap();
            assert!(
                batch.fits,
                "a large backlog must yield a bounded prefix: {batch:?}"
            );
            assert!(batch.complete);
            if batch.chunks == 0 {
                break;
            }
            assert!(batch.rows <= 32);
            batches += 1;
            sent_local_versions += batch.local_db_versions;
            assert!(batches <= 241, "bounded batches stopped making progress");
            let watermark = batch.watermark_db_version.unwrap();
            assert!(watermark > batch.start_db_version);
            if batches == 1 {
                assert!(batch.remaining);
                sqlx::query("INSERT INTO items (id, value) VALUES ('late', 'after preflight')")
                    .execute(&mut *connection)
                    .await
                    .unwrap();
            }
            let payloads: Vec<Vec<u8>> = sqlx::query_scalar(
                "SELECT payload FROM cloudsync_payload_chunks
                 WHERE since_db_version = ? AND until_db_version = ?",
            )
            .bind(batch.start_db_version)
            .bind(watermark)
            .fetch_all(&mut *connection)
            .await
            .unwrap();
            for payload in payloads {
                sqlx::query("SELECT cloudsync_payload_apply(?)")
                    .bind(payload)
                    .fetch_optional(&receiver)
                    .await
                    .unwrap();
            }
            let confirmed = NetworkStatus {
                last_optimistic_version: watermark,
                last_confirmed_version: watermark,
                gaps: Vec::new(),
                failures: NetworkStatusFailures::default(),
            };
            assert!(
                reconcile_confirmed_pending_payload(&mut connection, batch, &confirmed)
                    .await
                    .unwrap()
            );
        }
        assert!(batches > 1);
        assert_eq!(sent_local_versions, 241);
        let actual: Vec<(String, String)> =
            sqlx::query_as("SELECT id, value FROM items ORDER BY id")
                .fetch_all(&receiver)
                .await
                .unwrap();
        let expected: Vec<(String, String)> =
            sqlx::query_as("SELECT id, value FROM items ORDER BY id")
                .fetch_all(&mut *connection)
                .await
                .unwrap();
        assert_eq!(actual.len(), 241);
        assert_eq!(actual, expected);
        drop(connection);
        sender.close().await;
        receiver.close().await;
    }

    #[tokio::test]
    async fn pending_version_windows_respect_chunk_and_byte_limits() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let mut connection = pool.acquire().await.unwrap();
        sqlx::query(
            "CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL DEFAULT '')",
        )
        .execute(&mut *connection)
        .await
        .unwrap();
        init(&mut *connection, "items", None, None).await.unwrap();
        sqlx::query("SELECT cloudsync_set('payload_max_chunk_size', '262144')")
            .fetch_optional(&mut *connection)
            .await
            .unwrap();
        for index in 0..8 {
            sqlx::query("INSERT INTO items (id, value) VALUES (?, ?)")
                .bind(index.to_string())
                .bind("x".repeat(200 * 1024))
                .execute(&mut *connection)
                .await
                .unwrap();
        }
        let last_version = db_version(&mut *connection).await.unwrap();
        for (max_chunks, max_bytes) in [(2, u64::MAX), (8, 1_000)] {
            let batch = pending_payload_batch(&mut connection, max_chunks, u64::MAX, max_bytes)
                .await
                .unwrap();
            assert!(
                batch.fits && batch.complete && batch.remaining,
                "limits {max_chunks} / {max_bytes}: {batch:?}"
            );
            assert!(batch.chunks > 0 && batch.chunks <= max_chunks);
            assert!(batch.bytes <= max_bytes);
            assert!(batch.watermark_db_version.unwrap() < last_version);
        }
        drop(connection);
        pool.close().await;
    }

    #[tokio::test]
    async fn single_large_version_retries_after_restart_without_skipping_later_changes() {
        let directory = tempfile::tempdir().unwrap();
        let options = SqliteConnectOptions::new()
            .filename(directory.path().join("sender.db"))
            .create_if_missing(true);
        let (options, _) = apply(options).unwrap();
        let sender = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .unwrap();
        let (receiver_options, _) =
            apply(SqliteConnectOptions::from_str("sqlite::memory:").unwrap()).unwrap();
        let receiver = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(receiver_options)
            .await
            .unwrap();
        for pool in [&sender, &receiver] {
            sqlx::query(
                "CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL DEFAULT '')",
            )
            .execute(pool)
            .await
            .unwrap();
            init(pool, "items", None, None).await.unwrap();
        }
        // One statement gives every change the same indivisible database version.
        sqlx::query(
            "WITH RECURSIVE ids(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM ids WHERE id < 6564)
             INSERT INTO items SELECT CAST(id AS TEXT), printf('%02048d', id) FROM ids",
        )
        .execute(&sender)
        .await
        .unwrap();
        let first_version = db_version(&sender).await.unwrap();
        let mut connection = sender.acquire().await.unwrap();
        let only_version = pending_payload_batch(&mut connection, 8, 4096, 32 * 1024 * 1024)
            .await
            .unwrap();
        assert!(only_version.fits && only_version.complete && !only_version.remaining);
        assert_eq!(only_version.rows, 6564);
        drop(connection);
        sqlx::query("INSERT INTO items VALUES ('later', 'keep pending')")
            .execute(&sender)
            .await
            .unwrap();
        let mut connection = sender.acquire().await.unwrap();
        let (watermark, final_chunk): (i64, bool) = sqlx::query_as(
            "SELECT watermark_db_version, is_final FROM cloudsync_payload_chunks WHERE until_db_version = 0 LIMIT 1",
        )
        .fetch_one(&mut *connection)
        .await
        .unwrap();
        assert!(!final_chunk);
        assert!(watermark > first_version);
        let truncated_scan = pending_payload_batch(&mut connection, 8, 1, 32 * 1024 * 1024)
            .await
            .unwrap();
        assert!(truncated_scan.fits && truncated_scan.complete && truncated_scan.remaining);
        assert_eq!(truncated_scan.watermark_db_version, Some(first_version));
        let batch = pending_payload_batch(&mut connection, 8, 4096, 32 * 1024 * 1024)
            .await
            .unwrap();
        assert!(batch.fits && batch.complete && batch.remaining, "{batch:?}");
        assert_eq!(batch.rows, 6564);
        assert!(batch.chunks > 1 && batch.chunks <= 8);
        assert_eq!(batch.watermark_db_version, Some(first_version));

        for (max_chunks, max_bytes) in [(1, 32 * 1024 * 1024), (8, batch.bytes - 1)] {
            let rejected = pending_payload_batch(&mut connection, max_chunks, 4096, max_bytes)
                .await
                .unwrap();
            assert!(
                !rejected.fits,
                "hard limits must still reject: {rejected:?}"
            );
        }
        let payloads: Vec<Vec<u8>> = sqlx::query_scalar(
            "SELECT payload FROM cloudsync_payload_chunks WHERE until_db_version = ?",
        )
        .bind(first_version)
        .fetch_all(&mut *connection)
        .await
        .unwrap();
        // A chunk can be applied before its acknowledgement is lost.
        sqlx::query("SELECT cloudsync_payload_apply(?)")
            .bind(&payloads[0])
            .fetch_optional(&receiver)
            .await
            .unwrap();
        let mut confirmed = NetworkStatus {
            last_optimistic_version: first_version,
            last_confirmed_version: batch.start_db_version,
            gaps: Vec::new(),
            failures: NetworkStatusFailures::default(),
        };
        assert!(
            !reconcile_confirmed_pending_payload(&mut connection, batch, &confirmed)
                .await
                .unwrap()
        );
        drop(connection);
        sender.close().await;
        let sender = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        init(&sender, "items", None, None).await.unwrap();
        let mut connection = sender.acquire().await.unwrap();
        assert_eq!(
            pending_payload_batch(&mut connection, 8, 4096, 32 * 1024 * 1024)
                .await
                .unwrap(),
            batch
        );
        for payload in payloads {
            sqlx::query("SELECT cloudsync_payload_apply(?)")
                .bind(payload)
                .fetch_optional(&receiver)
                .await
                .unwrap();
        }
        confirmed.last_confirmed_version = first_version;
        assert!(
            reconcile_confirmed_pending_payload(&mut connection, batch, &confirmed)
                .await
                .unwrap()
        );
        let tail = pending_payload_batch(&mut connection, 8, 4096, 32 * 1024 * 1024)
            .await
            .unwrap();
        assert!(tail.fits && tail.complete && !tail.remaining);
        assert_eq!(tail.rows, 1);
        assert_eq!(tail.start_db_version, first_version);
        let payload: Vec<u8> = sqlx::query_scalar("SELECT payload FROM cloudsync_payload_chunks")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        sqlx::query("SELECT cloudsync_payload_apply(?)")
            .bind(payload)
            .fetch_optional(&receiver)
            .await
            .unwrap();
        let actual: Vec<(String, String)> =
            sqlx::query_as("SELECT id, value FROM items ORDER BY id")
                .fetch_all(&receiver)
                .await
                .unwrap();
        let expected: Vec<(String, String)> =
            sqlx::query_as("SELECT id, value FROM items ORDER BY id")
                .fetch_all(&mut *connection)
                .await
                .unwrap();
        assert_eq!(actual.len(), 6565);
        assert_eq!(actual, expected);
        drop(connection);
        sender.close().await;
        receiver.close().await;
    }

    #[tokio::test]
    async fn single_large_version_with_incomplete_chunks_is_rejected() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE cloudsync_settings (key TEXT, value TEXT);
             CREATE TABLE cloudsync_payload_chunks (
                 payload_size INTEGER, rows INTEGER, watermark_db_version INTEGER,
                 is_final BOOLEAN, db_version_min INTEGER, until_db_version INTEGER
             );
             INSERT INTO cloudsync_payload_chunks VALUES
                 (1024, 6564, 1, FALSE, 1, 0),
                 (1024, 6564, 1, FALSE, 1, 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let mut connection = pool.acquire().await.unwrap();
        let batch = pending_payload_batch(&mut connection, 8, 4096, 32 * 1024 * 1024)
            .await
            .unwrap();
        assert!(!batch.fits && !batch.complete);
        drop(connection);
        pool.close().await;
    }

    #[tokio::test]
    async fn bounded_native_send_keeps_unsent_versions_and_retries_failed_windows() {
        use std::io::{BufRead, Read, Write};
        use std::time::{Duration, Instant};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut requests = Vec::new();
            while requests.len() < 3 {
                let (mut stream, _) = match listener.accept() {
                    Ok(accepted) => accepted,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "native send did not reach the mock server"
                        );
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("mock accept failed: {error}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut reader = std::io::BufReader::new(&mut stream);
                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();
                let mut content_length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        content_length = value.trim().parse::<usize>().unwrap();
                    }
                }
                assert!(content_length < 128 * 1024);
                let mut body = vec![0; content_length];
                reader.read_exact(&mut body).unwrap();
                drop(reader);
                assert!(request_line.starts_with("POST "), "{request_line}");
                let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
                let failed = requests.is_empty();
                // Upstream adopts lastOptimisticVersion as the durable send
                // checkpoint, so confirm exactly the announced window.
                let version = body["dbVersionMax"].as_i64().unwrap();
                requests.push(body);
                let response = serde_json::json!({
                    "lastOptimisticVersion": version,
                    "lastConfirmedVersion": version,
                    "gaps": [],
                })
                .to_string();
                let status = if failed {
                    "503 Service Unavailable"
                } else {
                    "200 OK"
                };
                write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap();
                stream.flush().unwrap();
            }
            requests
        });

        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let mut connection = pool.acquire().await.unwrap();
        sqlx::query(
            "CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL DEFAULT '')",
        )
        .execute(&mut *connection)
        .await
        .unwrap();
        init(&mut *connection, "items", None, None).await.unwrap();
        sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
            .bind(endpoint)
            .bind("bounded-send-test")
            .fetch_optional(&mut *connection)
            .await
            .unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES ('first', 'before preflight')")
            .execute(&mut *connection)
            .await
            .unwrap();
        let first_version = db_version(&mut *connection).await.unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES ('later', 'after preflight')")
            .execute(&mut *connection)
            .await
            .unwrap();
        let last_version = db_version(&mut *connection).await.unwrap();

        assert!(
            network_send_changes_bounded(&mut *connection, 0)
                .await
                .is_err()
        );
        assert!(
            network_send_changes_bounded(&mut *connection, 1)
                .await
                .is_err()
        );
        let unchanged: String =
            sqlx::query_scalar("SELECT COALESCE((SELECT value FROM cloudsync_settings WHERE key = 'send_dbversion'), '0')")
                .fetch_one(&mut *connection)
                .await
                .unwrap();
        assert_eq!(unchanged, "0");

        let first = network_send_changes_bounded(&mut *connection, 1)
            .await
            .unwrap()
            .send
            .unwrap();
        assert_eq!(first.status, "out-of-sync");
        assert_eq!(first.local_version, last_version);
        assert_eq!(first.server_version, first_version);
        let last = network_send_changes_bounded(&mut *connection, 1)
            .await
            .unwrap()
            .send
            .unwrap();
        assert_eq!(last.status, "synced");
        assert_eq!(last.local_version, last_version);
        assert_eq!(last.server_version, last_version);

        let requests = server.join().unwrap();
        for request in [&requests[0], &requests[1]] {
            assert_eq!(request["dbVersionMin"], 1);
            assert_eq!(request["dbVersionMax"], first_version);
            assert_eq!(request["isFinal"], true);
            assert_eq!(request["chunkIndex"], 0);
        }
        assert_eq!(requests[2]["dbVersionMin"], first_version + 1);
        assert_eq!(requests[2]["dbVersionMax"], last_version);
        assert_eq!(requests[2]["isFinal"], true);
        drop(connection);
        pool.close().await;
    }

    #[tokio::test]
    async fn pending_payload_batch_ends_at_the_last_local_version() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let sender = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let peer_options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (peer_options, _) = apply(peer_options).unwrap();
        let peer = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(peer_options)
            .await
            .unwrap();
        for pool in [&sender, &peer] {
            sqlx::query(
                "CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL DEFAULT '')",
            )
            .execute(pool)
            .await
            .unwrap();
            init(pool, "items", None, None).await.unwrap();
        }
        let mut connection = sender.acquire().await.unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES ('one', 'local')")
            .execute(&mut *connection)
            .await
            .unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES ('two', 'local')")
            .execute(&mut *connection)
            .await
            .unwrap();
        let local_version = db_version(&mut *connection).await.unwrap();

        let mut peer_connection = peer.acquire().await.unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES ('remote', 'from peer')")
            .execute(&mut *peer_connection)
            .await
            .unwrap();
        let peer_payloads: Vec<Vec<u8>> =
            sqlx::query_scalar("SELECT payload FROM cloudsync_payload_chunks")
                .fetch_all(&mut *peer_connection)
                .await
                .unwrap();
        for payload in peer_payloads {
            sqlx::query("SELECT cloudsync_payload_apply(?)")
                .bind(payload)
                .fetch_optional(&mut *connection)
                .await
                .unwrap();
        }

        let batch = pending_payload_batch(&mut connection, 8, u64::MAX, 32 * 1024 * 1024)
            .await
            .unwrap();

        assert_eq!(batch.watermark_db_version, Some(local_version));
        assert_eq!(batch.local_db_versions, 2);
        drop(peer_connection);
        drop(connection);
        sender.close().await;
        peer.close().await;
    }

    #[tokio::test]
    async fn pending_payload_batch_counts_distinct_local_versions_in_one_table() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let mut connection = pool.acquire().await.unwrap();

        sqlx::query(
            "CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL DEFAULT '')",
        )
        .execute(&mut *connection)
        .await
        .unwrap();
        init(&mut *connection, "items", None, None).await.unwrap();
        // One statement: several rows share a single db_version.
        sqlx::query("INSERT INTO items (id, value) VALUES ('a', '1'), ('b', '2'), ('c', '3')")
            .execute(&mut *connection)
            .await
            .unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES ('d', '4')")
            .execute(&mut *connection)
            .await
            .unwrap();
        let last_version = db_version(&mut *connection).await.unwrap();

        let batch = pending_payload_batch(&mut connection, 8, u64::MAX, 32 * 1024 * 1024)
            .await
            .unwrap();

        assert_eq!(batch.local_db_versions, 2);
        assert_eq!(batch.watermark_db_version, Some(last_version));
        drop(connection);
        pool.close().await;
    }

    #[tokio::test]
    async fn pending_payload_batch_stops_after_the_first_chunk_over_the_limit() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let mut connection = pool.acquire().await.unwrap();

        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&mut *connection)
        .await
        .unwrap();
        init(&mut *connection, "items", None, None).await.unwrap();
        sqlx::query("SELECT cloudsync_set('payload_max_chunk_size', '262144')")
            .fetch_optional(&mut *connection)
            .await
            .unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES (?, ?)")
            .bind("large-value")
            .bind("x".repeat(2 * 1024 * 1024))
            .execute(&mut *connection)
            .await
            .unwrap();

        let batch = pending_payload_batch(&mut connection, 2, u64::MAX, u64::MAX)
            .await
            .unwrap();

        assert_eq!(batch.chunks, 3);
        assert!(!batch.complete);
        assert!(!batch.fits);
        drop(connection);
        pool.close().await;
    }

    #[tokio::test]
    async fn pending_payload_batch_reports_empty_fitting_stream() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let mut connection = pool.acquire().await.unwrap();

        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&mut *connection)
        .await
        .unwrap();
        init(&mut *connection, "items", None, None).await.unwrap();

        let batch = pending_payload_batch(&mut connection, 2, u64::MAX, 1024)
            .await
            .unwrap();

        assert_eq!(
            batch,
            PendingPayloadBatch {
                start_db_version: 0,
                watermark_db_version: None,
                chunks: 0,
                rows: 0,
                bytes: 0,
                complete: true,
                fits: true,
                remaining: false,
                local_db_versions: 0,
            }
        );
        drop(connection);
        pool.close().await;
    }

    #[tokio::test]
    async fn pending_payload_batch_rejects_a_single_oversized_chunk() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let mut connection = pool.acquire().await.unwrap();

        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&mut *connection)
        .await
        .unwrap();
        init(&mut *connection, "items", None, None).await.unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES ('item', 'payload')")
            .execute(&mut *connection)
            .await
            .unwrap();

        let batch = pending_payload_batch(&mut connection, 8, u64::MAX, 1)
            .await
            .unwrap();

        assert_eq!(batch.chunks, 1);
        assert!(batch.complete);
        assert!(!batch.fits);
        assert!(batch.bytes > 1);
        drop(connection);
        pool.close().await;
    }

    #[tokio::test]
    async fn confirmed_pending_payload_preserves_new_edits_when_retry_batch_grows() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let mut connection = pool.acquire().await.unwrap();

        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&mut *connection)
        .await
        .unwrap();
        init(&mut *connection, "items", None, None).await.unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES ('item', 'payload')")
            .execute(&mut *connection)
            .await
            .unwrap();
        sqlx::query("SELECT cloudsync_set('send_seq', '7')")
            .fetch_optional(&mut *connection)
            .await
            .unwrap();
        let batch = pending_payload_batch(&mut connection, 8, u64::MAX, 32 * 1024 * 1024)
            .await
            .unwrap();
        let watermark = batch.watermark_db_version.unwrap();
        let status: NetworkStatus = serde_json::from_value(serde_json::json!({
            "lastConfirmedVersion": watermark + 100,
            "gaps": [],
            "failures": {"apply": null}
        }))
        .unwrap();

        assert!(
            reconcile_confirmed_pending_payload(&mut connection, batch, &status)
                .await
                .unwrap()
        );
        let cursors: (String, String) = sqlx::query_as(
            "SELECT
                MAX(CASE WHEN key = 'send_dbversion' THEN value END),
                MAX(CASE WHEN key = 'send_seq' THEN value END)
             FROM cloudsync_settings",
        )
        .fetch_one(&mut *connection)
        .await
        .unwrap();
        assert_eq!(cursors, (watermark.to_string(), "7".to_string()));

        sqlx::query("INSERT INTO items VALUES ('confirmed-prefix', 'acknowledgement lost')")
            .execute(&mut *connection)
            .await
            .unwrap();
        let sent = pending_payload_batch(&mut connection, 8, u64::MAX, 32 * 1024 * 1024)
            .await
            .unwrap();
        let confirmed_version = sent.watermark_db_version.unwrap();
        sqlx::query("INSERT INTO items VALUES ('pending-tail', 'written before retry')")
            .execute(&mut *connection)
            .await
            .unwrap();
        let retry = pending_payload_batch(&mut connection, 8, u64::MAX, 32 * 1024 * 1024)
            .await
            .unwrap();
        assert!(retry.watermark_db_version.unwrap() > confirmed_version);
        let confirmed_prefix = NetworkStatus {
            last_optimistic_version: confirmed_version,
            last_confirmed_version: confirmed_version,
            gaps: Vec::new(),
            failures: NetworkStatusFailures::default(),
        };
        assert!(
            reconcile_confirmed_pending_payload(&mut connection, retry, &confirmed_prefix)
                .await
                .unwrap()
        );
        let tail = pending_payload_batch(&mut connection, 8, u64::MAX, 32 * 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(tail.start_db_version, confirmed_version);
        assert_eq!(tail.watermark_db_version, retry.watermark_db_version);
        assert_eq!(tail.rows, 1);
        assert_eq!(tail.local_db_versions, 1);
        assert!(tail.complete && tail.fits);
        drop(connection);
        pool.close().await;
    }

    #[tokio::test]
    async fn confirmed_pending_payload_rejects_gaps_and_cursor_changes() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let mut connection = pool.acquire().await.unwrap();

        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&mut *connection)
        .await
        .unwrap();
        init(&mut *connection, "items", None, None).await.unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES ('item', 'payload')")
            .execute(&mut *connection)
            .await
            .unwrap();
        let batch = pending_payload_batch(&mut connection, 8, u64::MAX, 32 * 1024 * 1024)
            .await
            .unwrap();
        let watermark = batch.watermark_db_version.unwrap();
        let status_with_gap = NetworkStatus {
            last_optimistic_version: watermark,
            last_confirmed_version: watermark,
            gaps: vec![serde_json::json!({"from": 1, "to": 1})],
            failures: NetworkStatusFailures::default(),
        };
        assert!(
            !reconcile_confirmed_pending_payload(&mut connection, batch, &status_with_gap)
                .await
                .unwrap()
        );

        sqlx::query("SELECT cloudsync_set('send_dbversion', '1')")
            .fetch_optional(&mut *connection)
            .await
            .unwrap();
        let confirmed = NetworkStatus {
            gaps: Vec::new(),
            ..status_with_gap
        };
        assert!(
            !reconcile_confirmed_pending_payload(&mut connection, batch, &confirmed)
                .await
                .unwrap()
        );
        drop(connection);
        pool.close().await;
    }

    #[test]
    fn network_status_accepts_confirmed_version_without_optimistic_version() {
        let status: NetworkStatus = serde_json::from_str(
            r#"{
                "lastOptimisticVersion": 8,
                "lastConfirmedVersion": 7,
                "gaps": [],
                "failures": {"apply": null, "check": {"code": "pending"}}
            }"#,
        )
        .unwrap();

        assert_eq!(status.last_optimistic_version, 8);
        assert_eq!(status.last_confirmed_version, 7);
        assert!(status.gaps.is_empty());
        assert!(status.failures.apply.is_none());
        assert!(status.failures.check.is_some());

        let confirmed: NetworkStatus =
            serde_json::from_str(r#"{"lastConfirmedVersion":7,"gaps":[]}"#).unwrap();
        assert_eq!(confirmed.last_optimistic_version, 7);
        assert_eq!(confirmed.last_confirmed_version, 7);

        let enveloped: NetworkStatus = serde_json::from_str(
            r#"{
                "data": {
                    "lastOptimisticVersion": 9,
                    "lastConfirmedVersion": 9,
                    "gaps": null,
                    "failures": {"apply": null, "check": null}
                }
            }"#,
        )
        .unwrap();
        assert_eq!(enveloped.last_optimistic_version, 9);
        assert_eq!(enveloped.last_confirmed_version, 9);
        assert!(enveloped.gaps.is_empty());

        assert!(
            serde_json::from_str::<NetworkStatus>(
                r#"{"lastOptimisticVersion":8,"lastConfirmedVersion":7}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<NetworkStatus>(r#"{"lastOptimisticVersion":8,"gaps":[]}"#)
                .is_err()
        );
        assert!(
            serde_json::from_str::<NetworkStatus>(
                r#"{"data":{"lastConfirmedVersion":7,"gaps":{}}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn network_status_accepts_null_failures_for_direct_and_enveloped_responses() {
        for input in [
            r#"{"lastConfirmedVersion":9,"gaps":null,"failures":null}"#,
            r#"{"data":{"lastConfirmedVersion":9,"gaps":null,"failures":null}}"#,
        ] {
            let status: NetworkStatus = serde_json::from_str(input).unwrap();
            assert_eq!(status.last_optimistic_version, 9);
            assert_eq!(status.last_confirmed_version, 9);
            assert!(status.gaps.is_empty());
            assert_eq!(status.failures, NetworkStatusFailures::default());
        }
    }

    #[test]
    fn network_status_rejects_negative_direct_and_enveloped_versions() {
        for input in [
            r#"{"lastOptimisticVersion":-1,"lastConfirmedVersion":0,"gaps":[]}"#,
            r#"{"lastOptimisticVersion":0,"lastConfirmedVersion":-1,"gaps":[]}"#,
            r#"{"data":{"lastConfirmedVersion":-1,"gaps":null}}"#,
        ] {
            assert!(serde_json::from_str::<NetworkStatus>(input).is_err());
        }
    }

    #[tokio::test]
    async fn receive_version_reset_preserves_outbound_cursor() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let mut connection = pool.acquire().await.unwrap();

        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&mut *connection)
        .await
        .unwrap();
        init(&mut *connection, "items", None, None).await.unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES ('local', 'pending')")
            .execute(&mut *connection)
            .await
            .unwrap();
        let local_changes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloudsync_changes")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        assert!(local_changes > 0);

        sqlx::query(
            "SELECT
                cloudsync_set('check_dbversion', '23'),
                cloudsync_set('check_seq', '5'),
                cloudsync_set('send_dbversion', '17'),
                cloudsync_set('send_seq', '3')",
        )
        .fetch_optional(&mut *connection)
        .await
        .unwrap();

        network_reset_receive_version(&mut connection)
            .await
            .unwrap();

        let cursors: (String, String, String, String) = sqlx::query_as(
            "SELECT
                MAX(CASE WHEN key = 'check_dbversion' THEN value END),
                MAX(CASE WHEN key = 'check_seq' THEN value END),
                MAX(CASE WHEN key = 'send_dbversion' THEN value END),
                MAX(CASE WHEN key = 'send_seq' THEN value END)
             FROM cloudsync_settings",
        )
        .fetch_one(&mut *connection)
        .await
        .unwrap();
        assert_eq!(cursors, ("0".into(), "0".into(), "17".into(), "3".into()));

        drop(connection);
        pool.close().await;
    }

    #[tokio::test]
    async fn write_filter_can_use_a_local_workspace_scope_table() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE writable_workspaces (
               allowed_workspace_id TEXT PRIMARY KEY NOT NULL
             )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO writable_workspaces (allowed_workspace_id) VALUES ('personal')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE items (
               id TEXT PRIMARY KEY NOT NULL,
               workspace_id TEXT NOT NULL DEFAULT '',
               title TEXT NOT NULL DEFAULT ''
             )",
        )
        .execute(&pool)
        .await
        .unwrap();
        init(&pool, "items", None, None).await.unwrap();
        set_filter(
            &pool,
            "items",
            "workspace_id IN (SELECT allowed_workspace_id FROM writable_workspaces)",
        )
        .await
        .unwrap();
        let baseline_metadata_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM items_cloudsync")
                .fetch_one(&pool)
                .await
                .unwrap();

        sqlx::query(
            "INSERT INTO items (id, workspace_id, title)
             VALUES ('shared', 'shared', 'Shared')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let shared_metadata_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items_cloudsync")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(shared_metadata_count, baseline_metadata_count);

        sqlx::query(
            "INSERT INTO items (id, workspace_id, title)
             VALUES ('personal', 'personal', 'Personal')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let personal_metadata_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM items_cloudsync")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(personal_metadata_count > baseline_metadata_count);
        pool.close().await;
    }

    #[tokio::test]
    async fn reopens_initialized_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cloudsync.db");

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE items (
                id INTEGER PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        init(&pool, "items", None, Some(1)).await.unwrap();
        pool.close().await;

        let options = SqliteConnectOptions::new().filename(&path);
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();

        assert_eq!(version(&pool).await.unwrap(), CLOUDSYNC_VERSION);
        pool.close().await;
    }

    #[tokio::test]
    async fn terminates_each_pool_connection_before_close() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cloudsync.db");
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .unwrap();

        let mut connections = Vec::new();
        for index in 0..4 {
            let mut connection = pool.acquire().await.unwrap();
            let table = format!("items_{index}");
            let sql = format!(
                "CREATE TABLE {table} (id INTEGER PRIMARY KEY NOT NULL, value TEXT NOT NULL DEFAULT '')"
            );
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .execute(&mut *connection)
                .await
                .unwrap();
            init(&mut *connection, &table, None, Some(1)).await.unwrap();
            connections.push(connection);
        }

        for connection in connections {
            connection.close().await.unwrap();
        }
        pool.close().await;
    }
}

#![deny(unsafe_code)]

mod api;
mod bundle;
mod close;
mod error;
mod network;

use std::path::PathBuf;

use sqlx::sqlite::SqliteConnectOptions;

pub use api::{
    CloudsyncConnectionInitializer, CloudsyncTableSpec, begin_alter, cleanup, commit_alter,
    db_version, disable, enable, init, is_enabled, set_filter, siteid, terminate, uuid, version,
};
pub use bundle::bundled_extension_path;
pub use close::install_transaction_observer;
pub use error::{Error, ErrorKind};
pub use network::{
    NetworkReceiveResult, NetworkResult, NetworkSendResult, NetworkStatus, NetworkStatusFailures,
    PendingPayloadBatch, network_check_changes, network_cleanup, network_has_unsent_changes,
    network_init, network_logout, network_receive_changes, network_reset_receive_version,
    network_reset_sync_version, network_send_changes, network_set_apikey, network_set_token,
    network_status, network_sync, pending_payload_batch, reconcile_confirmed_pending_payload,
};

pub const CLOUDSYNC_VERSION: &str = "1.1.2";

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
    use std::str::FromStr;

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

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    #[tokio::test]
    async fn native_http_request_deadline_is_enforced() {
        const CHILD_ENV: &str = "ANARLOG_CLOUDSYNC_TIMEOUT_TEST_CHILD";

        if std::env::var_os(CHILD_ENV).is_none() {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("native_http_request_deadline_is_enforced")
                .arg("--nocapture")
                .env(CHILD_ENV, "1")
                .env("CLOUDSYNC_CURL_CONNECT_TIMEOUT_MS", "100")
                .env("CLOUDSYNC_CURL_TIMEOUT_MS", "250")
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .spawn()
                .unwrap();
            let watchdog = std::time::Instant::now() + std::time::Duration::from_secs(10);

            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success(), "native timeout child test failed");
                    return;
                }
                if std::time::Instant::now() >= watchdog {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("native CloudSync request exceeded the timeout watchdog");
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        }

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

        let started = std::time::Instant::now();
        let error = network_receive_changes(&pool, Some(1)).await.unwrap_err();
        let elapsed = started.elapsed();

        assert!(
            elapsed < std::time::Duration::from_secs(2),
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
    async fn confirmed_pending_payload_advances_only_to_the_preflighted_watermark() {
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

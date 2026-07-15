#![deny(unsafe_code)]

mod api;
mod bundle;
mod close;
mod error;
mod network;

use std::path::PathBuf;

use sqlx::sqlite::SqliteConnectOptions;

pub use api::{
    begin_alter, cleanup, commit_alter, db_version, disable, enable, init, is_enabled, siteid,
    terminate, uuid, version,
};
pub use bundle::bundled_extension_path;
pub use close::install_transaction_observer;
pub use error::{Error, ErrorKind};
pub use network::{
    NetworkReceiveResult, NetworkResult, NetworkSendResult, network_check_changes, network_cleanup,
    network_has_unsent_changes, network_init, network_logout, network_receive_changes,
    network_reset_sync_version, network_send_changes, network_set_apikey, network_set_token,
    network_sync,
};

pub const CLOUDSYNC_VERSION: &str = "1.1.2";

pub fn apply(options: SqliteConnectOptions) -> Result<(SqliteConnectOptions, PathBuf), Error> {
    close::install_terminate_on_close()?;
    let extension_path = bundled_extension_path()?;

    #[allow(unsafe_code)]
    let options = unsafe { options.extension(extension_path.to_string_lossy().into_owned()) };

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

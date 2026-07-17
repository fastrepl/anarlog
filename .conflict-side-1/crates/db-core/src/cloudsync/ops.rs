use sqlx::pool::PoolConnection;
use sqlx::{Executor, Sqlite, SqliteConnection};
use tokio::sync::MutexGuard;

use super::{CloudsyncAuth, CloudsyncTableSpec};
use crate::Db;

impl Db {
    async fn lock_cloudsync_connection(
        &self,
    ) -> Result<MutexGuard<'_, Option<PoolConnection<Sqlite>>>, hypr_cloudsync::Error> {
        let mut connection = self.cloudsync_connection.lock().await;
        if connection.is_none() {
            *connection = Some(self.pool.acquire().await?);
        }
        Ok(connection)
    }

    fn release_single_pool_connection(
        &self,
        connection: &mut MutexGuard<'_, Option<PoolConnection<Sqlite>>>,
    ) {
        if self.pool.options().get_max_connections() == 1 {
            connection.take();
        }
    }

    pub fn cloudsync_enabled(&self) -> bool {
        self.cloudsync_enabled
    }

    pub fn has_cloudsync(&self) -> bool {
        self.cloudsync_path.is_some()
    }

    pub fn cloudsync_path(&self) -> Option<&std::path::Path> {
        self.cloudsync_path.as_deref()
    }

    pub async fn cloudsync_version(&self) -> Result<String, hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::version(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_init(
        &self,
        table_name: &str,
        crdt_algo: Option<&str>,
        init_flags: Option<i64>,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::init(
            &mut **connection.as_mut().unwrap(),
            table_name,
            crdt_algo,
            init_flags,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_set_filter(
        &self,
        table_name: &str,
        filter_expression: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::set_filter(
            &mut **connection.as_mut().unwrap(),
            table_name,
            filter_expression,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub(crate) async fn cloudsync_init_enabled_tables(
        &self,
        tables: &[CloudsyncTableSpec],
    ) -> Result<(), hypr_cloudsync::Error> {
        if !tables.iter().any(|table| table.enabled) {
            return Ok(());
        }

        let mut pinned = self.lock_cloudsync_connection().await?;
        let mut connections = Vec::new();
        for _ in 1..self.pool.options().get_max_connections() {
            match self.pool.acquire().await {
                Ok(connection) => connections.push(connection),
                Err(error) => {
                    for connection in connections {
                        let _ = connection.close().await;
                    }
                    self.release_single_pool_connection(&mut pinned);
                    return Err(error.into());
                }
            }
        }

        let result = async {
            init_enabled_tables(pinned.as_mut().unwrap(), tables).await?;
            for connection in &mut connections {
                init_enabled_tables(connection, tables).await?;
            }
            Ok(())
        }
        .await;

        if result.is_ok() {
            self.cloudsync_initializer.replace_tables(
                tables
                    .iter()
                    .filter(|table| table.enabled)
                    .cloned()
                    .collect(),
            );
        }

        self.release_single_pool_connection(&mut pinned);
        if result.is_err() {
            for connection in connections {
                let _ = connection.close().await;
            }
        }

        result
    }

    pub async fn cloudsync_network_init(
        &self,
        connection_string: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_init(&mut **connection.as_mut().unwrap(), connection_string)
                .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_set_apikey(
        &self,
        api_key: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_set_apikey(&mut **connection.as_mut().unwrap(), api_key).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_set_token(
        &self,
        token: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_set_token(&mut **connection.as_mut().unwrap(), token).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_begin_alter(
        &self,
        table_name: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            cloudsync_begin_alter_on(&mut **connection.as_mut().unwrap(), table_name).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_commit_alter(
        &self,
        table_name: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            cloudsync_commit_alter_on(&mut **connection.as_mut().unwrap(), table_name).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_cleanup(&self, table_name: &str) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::cleanup(&mut **connection.as_mut().unwrap(), table_name).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_terminate(&self) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::terminate(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub(crate) async fn cloudsync_terminate_and_close(&self) -> Result<(), hypr_cloudsync::Error> {
        self.cloudsync_initializer.clear();
        let mut pinned = self.lock_cloudsync_connection().await?;
        let mut connections = Vec::new();
        for _ in 1..self.pool.options().get_max_connections() {
            match self.pool.acquire().await {
                Ok(connection) => connections.push(connection),
                Err(error) => {
                    connections.push(pinned.take().unwrap());
                    drop(pinned);
                    if let Err(close_error) = close_pool_connections(connections).await {
                        tracing::warn!(%close_error, "failed to close cloudsync connections after pool acquisition failure");
                    }
                    return Err(error.into());
                }
            }
        }

        let mut terminate_error = None;
        if let Err(error) = hypr_cloudsync::terminate(&mut **pinned.as_mut().unwrap()).await {
            terminate_error = Some(error);
        }
        for connection in &mut connections {
            if let Err(error) = hypr_cloudsync::terminate(&mut **connection).await
                && terminate_error.is_none()
            {
                terminate_error = Some(error);
            }
        }

        connections.push(pinned.take().unwrap());
        drop(pinned);
        let close_result = close_pool_connections(connections).await;

        if let Some(error) = terminate_error {
            return Err(error);
        }
        close_result
    }

    pub(crate) async fn cloudsync_close_connection(&self) -> Result<(), hypr_cloudsync::Error> {
        let connection = self.cloudsync_connection.lock().await.take();
        match connection {
            Some(connection) => connection
                .close()
                .await
                .map_err(hypr_cloudsync::Error::from),
            None => Ok(()),
        }
    }

    pub async fn cloudsync_network_cleanup(&self) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::network_cleanup(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_has_unsent_changes(
        &self,
    ) -> Result<bool, hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_has_unsent_changes(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_send_changes(
        &self,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_send_changes(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_receive_changes(
        &self,
        max_chunks: Option<i64>,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::network_receive_changes(
            &mut **connection.as_mut().unwrap(),
            max_chunks,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_check_changes(
        &self,
        max_chunks: Option<i64>,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        self.cloudsync_network_receive_changes(max_chunks).await
    }

    pub async fn cloudsync_network_reset_sync_version(&self) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_reset_sync_version(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_logout(&self) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::network_logout(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_sync(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_sync(&mut **connection.as_mut().unwrap(), wait_ms, max_retries)
                .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub(crate) async fn apply_cloudsync_auth(
        &self,
        auth: &CloudsyncAuth,
    ) -> Result<(), hypr_cloudsync::Error> {
        match auth {
            CloudsyncAuth::None => Ok(()),
            CloudsyncAuth::ApiKey { api_key } => self.cloudsync_network_set_apikey(api_key).await,
            CloudsyncAuth::Token { token } => self.cloudsync_network_set_token(token).await,
        }
    }
}

async fn init_enabled_tables(
    connection: &mut SqliteConnection,
    tables: &[CloudsyncTableSpec],
) -> Result<(), hypr_cloudsync::Error> {
    for table in tables.iter().filter(|table| table.enabled) {
        hypr_cloudsync::init(
            &mut *connection,
            &table.table_name,
            table.crdt_algo.as_deref(),
            table.init_flags,
        )
        .await?;
    }

    Ok(())
}

pub async fn cloudsync_begin_alter_on<'e, E>(
    executor: E,
    table_name: &str,
) -> Result<(), hypr_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    hypr_cloudsync::begin_alter(executor, table_name).await
}

pub async fn cloudsync_is_enabled_on<'e, E>(
    executor: E,
    table_name: &str,
) -> Result<bool, hypr_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    hypr_cloudsync::is_enabled(executor, table_name).await
}

pub async fn cloudsync_commit_alter_on<'e, E>(
    executor: E,
    table_name: &str,
) -> Result<(), hypr_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    hypr_cloudsync::commit_alter(executor, table_name).await
}

async fn close_pool_connections(
    connections: Vec<PoolConnection<Sqlite>>,
) -> Result<(), hypr_cloudsync::Error> {
    let mut first_error = None;
    for connection in connections {
        if let Err(error) = connection.close().await
            && first_error.is_none()
        {
            first_error = Some(error.into());
        }
    }

    first_error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;

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
        let mut task =
            tokio::spawn(async move { task_db.cloudsync_network_sync(None, None).await });

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
}

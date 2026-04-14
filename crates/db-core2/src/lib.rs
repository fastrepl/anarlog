mod cloudsync;
mod pool;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use hypr_cloudsync::Error;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;

use crate::cloudsync::CloudsyncRuntimeState;
pub use crate::cloudsync::{
    CloudsyncAuth, CloudsyncOpenMode, CloudsyncRuntimeConfig, CloudsyncRuntimeError,
    CloudsyncStatus, CloudsyncTableSpec,
};
use crate::pool::connect_pool;
pub use crate::pool::{DbPool, TableChange, TableChangeKind};

#[derive(Clone, Copy, Debug)]
pub enum DbStorage<'a> {
    Local(&'a Path),
    Memory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationFailurePolicy {
    Fail,
    Recreate,
}

#[derive(Clone, Copy, Debug)]
pub struct DbOpenOptions<'a> {
    pub storage: DbStorage<'a>,
    pub cloudsync_open_mode: CloudsyncOpenMode,
    pub journal_mode_wal: bool,
    pub foreign_keys: bool,
    pub max_connections: Option<u32>,
    pub migration_failure_policy: MigrationFailurePolicy,
}

#[derive(Debug, thiserror::Error)]
pub enum DbOpenError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Cloudsync(#[from] hypr_cloudsync::Error),
    #[error("migration failed: {0}")]
    Migration(String),
    #[error("failed to recreate database after migration failure: {0}")]
    RecreateFailed(String),
}

pub type ManagedDb = std::sync::Arc<Db3>;

type BoxedMigrationFuture<'a, E> = Pin<Box<dyn Future<Output = Result<(), E>> + Send + 'a>>;

const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Db3 {
    pub(crate) cloudsync_open_mode: CloudsyncOpenMode,
    pub(crate) cloudsync_path: Option<PathBuf>,
    pub(crate) cloudsync_runtime: Arc<Mutex<CloudsyncRuntimeState>>,
    pub(crate) pool: DbPool,
}

impl std::fmt::Debug for Db3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let runtime = self.cloudsync_runtime.lock().unwrap();
        f.debug_struct("Db3")
            .field("cloudsync_open_mode", &self.cloudsync_open_mode)
            .field("cloudsync_path", &self.cloudsync_path)
            .field("cloudsync_runtime", &*runtime)
            .finish_non_exhaustive()
    }
}

impl Db3 {
    pub async fn open_with_migrate<F, E>(
        options: DbOpenOptions<'_>,
        migrate: F,
    ) -> Result<Self, DbOpenError>
    where
        F: for<'a> Fn(&'a SqlitePool) -> BoxedMigrationFuture<'a, E>,
        E: std::fmt::Display,
    {
        match try_open_with_migrate(&options, &migrate).await {
            Ok(db) => Ok(db),
            Err(DbOpenError::Migration(message))
                if matches!(
                    options.migration_failure_policy,
                    MigrationFailurePolicy::Recreate
                ) =>
            {
                tracing::warn!("database migration failed, recreating fresh database: {message}");
                recreate_storage(&options)?;
                try_open_with_migrate(&options, &migrate)
                    .await
                    .map_err(|error| DbOpenError::RecreateFailed(error.to_string()))
            }
            Err(error) => Err(error),
        }
    }

    pub async fn connect_local(path: impl AsRef<Path>) -> Result<Self, Error> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let options = apply_internal_connect_policy(SqliteConnectOptions::new())
            .filename(path)
            .create_if_missing(true);
        let (options, cloudsync_path) = hypr_cloudsync::apply(options)?;
        let pool = connect_pool(options, None).await.map_err(Error::from)?;

        Ok(Self {
            cloudsync_open_mode: CloudsyncOpenMode::Enabled,
            cloudsync_path: Some(cloudsync_path),
            cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
            pool,
        })
    }

    pub async fn connect_memory() -> Result<Self, Error> {
        let options =
            apply_internal_connect_policy(SqliteConnectOptions::from_str("sqlite::memory:")?);
        let (options, cloudsync_path) = hypr_cloudsync::apply(options)?;
        let pool = connect_pool(options, Some(1)).await.map_err(Error::from)?;

        Ok(Self {
            cloudsync_open_mode: CloudsyncOpenMode::Enabled,
            cloudsync_path: Some(cloudsync_path),
            cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
            pool,
        })
    }

    pub async fn connect_local_plain(path: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(sqlx::Error::Io)?;
        }
        let options = apply_internal_connect_policy(SqliteConnectOptions::new())
            .filename(path)
            .create_if_missing(true)
            .pragma("foreign_keys", "ON");
        let pool = connect_pool(options, None).await?;

        Ok(Self {
            cloudsync_open_mode: CloudsyncOpenMode::Disabled,
            cloudsync_path: None,
            cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
            pool,
        })
    }

    pub async fn connect_memory_plain() -> Result<Self, sqlx::Error> {
        let options =
            apply_internal_connect_policy(SqliteConnectOptions::from_str("sqlite::memory:")?)
                .pragma("foreign_keys", "ON");
        let pool = connect_pool(options, Some(1)).await?;

        Ok(Self {
            cloudsync_open_mode: CloudsyncOpenMode::Disabled,
            cloudsync_path: None,
            cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
            pool,
        })
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }
}

async fn try_open_with_migrate<F, E>(
    options: &DbOpenOptions<'_>,
    migrate: &F,
) -> Result<Db3, DbOpenError>
where
    F: for<'a> Fn(&'a SqlitePool) -> BoxedMigrationFuture<'a, E>,
    E: std::fmt::Display,
{
    let db = connect_with_options(options).await?;

    if let Err(error) = migrate(db.pool()).await {
        db.pool.clone().close().await;
        return Err(DbOpenError::Migration(error.to_string()));
    }

    Ok(db)
}

async fn connect_with_options(options: &DbOpenOptions<'_>) -> Result<Db3, DbOpenError> {
    let mut connect_options = match options.storage {
        DbStorage::Local(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            apply_internal_connect_policy(SqliteConnectOptions::new())
                .filename(path)
                .create_if_missing(true)
        }
        DbStorage::Memory => {
            apply_internal_connect_policy(SqliteConnectOptions::from_str("sqlite::memory:")?)
        }
    };

    if options.journal_mode_wal {
        connect_options = connect_options.pragma("journal_mode", "WAL");
    }
    if options.foreign_keys {
        connect_options = connect_options.pragma("foreign_keys", "ON");
    }

    let (connect_options, cloudsync_path) =
        if options.cloudsync_open_mode == CloudsyncOpenMode::Enabled {
            let (connect_options, cloudsync_path) = hypr_cloudsync::apply(connect_options)?;
            (connect_options, Some(cloudsync_path))
        } else {
            (connect_options, None)
        };

    let max_connections = match options.storage {
        DbStorage::Memory => Some(1),
        DbStorage::Local(_) => options.max_connections,
    };
    let pool = connect_pool(connect_options, max_connections).await?;

    Ok(Db3 {
        cloudsync_open_mode: options.cloudsync_open_mode,
        cloudsync_path,
        cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
        pool,
    })
}

fn apply_internal_connect_policy(connect_options: SqliteConnectOptions) -> SqliteConnectOptions {
    connect_options.busy_timeout(SQLITE_BUSY_TIMEOUT)
}

fn recreate_storage(options: &DbOpenOptions<'_>) -> Result<(), DbOpenError> {
    match options.storage {
        DbStorage::Local(path) => {
            wipe_db_file(path);
            if options.cloudsync_open_mode == CloudsyncOpenMode::Enabled {
                let connect_options = SqliteConnectOptions::new().filename(path);
                let (_, cloudsync_path) = hypr_cloudsync::apply(connect_options)?;
                wipe_db_file(&cloudsync_path);
            }
        }
        DbStorage::Memory => {}
    }

    Ok(())
}

fn wipe_db_file(path: &Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let file = PathBuf::from(format!("{}{suffix}", path.display()));
        if file.exists() {
            let _ = std::fs::remove_file(file);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn test_cloudsync_config() -> CloudsyncRuntimeConfig {
        CloudsyncRuntimeConfig {
            connection_string: "sqlitecloud://demo.invalid/app.db?apikey=demo".to_string(),
            auth: CloudsyncAuth::None,
            tables: vec![CloudsyncTableSpec {
                table_name: "test_sync".to_string(),
                crdt_algo: None,
                force_init: None,
                enabled: true,
            }],
            sync_interval_ms: 30_000,
            wait_ms: Some(500),
            max_retries: Some(1),
        }
    }

    #[tokio::test]
    async fn connect_local_plain_creates_parent_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("nonexistent").join("nested").join("app.db");
        let db = Db3::connect_local_plain(&db_path).await.unwrap();
        assert!(db_path.exists());
        drop(db);
    }

    #[tokio::test]
    async fn open_with_migrate_recreates_local_db_when_requested() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("app.db");
        let attempts = AtomicUsize::new(0);

        let db = Db3::open_with_migrate(
            DbOpenOptions {
                storage: DbStorage::Local(&db_path),
                cloudsync_open_mode: CloudsyncOpenMode::Disabled,
                journal_mode_wal: true,
                foreign_keys: true,
                max_connections: Some(1),
                migration_failure_policy: MigrationFailurePolicy::Recreate,
            },
            |pool| {
                let n = attempts.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    if n == 0 {
                        sqlx::query("CREATE TABLE broken (id TEXT PRIMARY KEY NOT NULL)")
                            .execute(pool)
                            .await
                            .unwrap();
                        Err("boom")
                    } else {
                        sqlx::query("CREATE TABLE fresh (id TEXT PRIMARY KEY NOT NULL)")
                            .execute(pool)
                            .await
                            .unwrap();
                        Ok::<(), &'static str>(())
                    }
                })
            },
        )
        .await
        .unwrap();

        let tables: Vec<String> = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(db.pool().as_ref())
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.0)
        .collect();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(tables, vec!["fresh"]);
    }

    #[tokio::test]
    async fn open_with_migrate_returns_migration_error_when_fail_policy_is_used() {
        let error = Db3::open_with_migrate(
            DbOpenOptions {
                storage: DbStorage::Memory,
                cloudsync_open_mode: CloudsyncOpenMode::Disabled,
                journal_mode_wal: false,
                foreign_keys: true,
                max_connections: Some(1),
                migration_failure_policy: MigrationFailurePolicy::Fail,
            },
            |_pool| Box::pin(async { Err::<(), _>("nope") }),
        )
        .await
        .unwrap_err();

        assert!(matches!(error, DbOpenError::Migration(message) if message == "nope"));
    }

    #[tokio::test]
    async fn open_with_migrate_returns_recreate_failed_when_retry_also_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("app.db");
        let attempts = AtomicUsize::new(0);

        let error = Db3::open_with_migrate(
            DbOpenOptions {
                storage: DbStorage::Local(&db_path),
                cloudsync_open_mode: CloudsyncOpenMode::Disabled,
                journal_mode_wal: true,
                foreign_keys: true,
                max_connections: Some(1),
                migration_failure_policy: MigrationFailurePolicy::Recreate,
            },
            |pool| {
                let n = attempts.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    let table_name = if n == 0 {
                        "first_attempt"
                    } else {
                        "second_attempt"
                    };
                    let sql = format!("CREATE TABLE {table_name} (id TEXT PRIMARY KEY NOT NULL)");
                    sqlx::query(&sql).execute(pool).await.unwrap();
                    Err::<(), &'static str>("still broken")
                })
            },
        )
        .await
        .unwrap_err();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert!(
            matches!(error, DbOpenError::RecreateFailed(message) if message == "migration failed: still broken")
        );
    }

    #[tokio::test]
    async fn open_with_migrate_applies_requested_pragmas() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("app.db");

        let db = Db3::open_with_migrate(
            DbOpenOptions {
                storage: DbStorage::Local(&db_path),
                cloudsync_open_mode: CloudsyncOpenMode::Disabled,
                journal_mode_wal: true,
                foreign_keys: true,
                max_connections: Some(1),
                migration_failure_policy: MigrationFailurePolicy::Fail,
            },
            |_pool| Box::pin(async { Ok::<(), sqlx::Error>(()) }),
        )
        .await
        .unwrap();

        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(db.pool().as_ref())
            .await
            .unwrap();
        let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(db.pool().as_ref())
            .await
            .unwrap();
        let busy_timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(db.pool().as_ref())
            .await
            .unwrap();

        assert_eq!(foreign_keys, 1);
        assert_eq!(journal_mode.to_lowercase(), "wal");
        assert_eq!(busy_timeout, SQLITE_BUSY_TIMEOUT.as_millis() as i64);
    }

    #[tokio::test]
    async fn disabled_open_mode_keeps_cloudsync_inert() {
        let db = Db3::open_with_migrate(
            DbOpenOptions {
                storage: DbStorage::Memory,
                cloudsync_open_mode: CloudsyncOpenMode::Disabled,
                journal_mode_wal: false,
                foreign_keys: true,
                max_connections: Some(1),
                migration_failure_policy: MigrationFailurePolicy::Fail,
            },
            |_pool| Box::pin(async { Ok::<(), sqlx::Error>(()) }),
        )
        .await
        .unwrap();

        assert_eq!(db.cloudsync_open_mode(), CloudsyncOpenMode::Disabled);
        assert!(!db.has_cloudsync());

        db.cloudsync_configure(test_cloudsync_config()).unwrap();
        db.cloudsync_start().await.unwrap();

        let status = db.cloudsync_status().await.unwrap();
        assert!(status.configured);
        assert!(!status.extension_loaded);
        assert!(!status.running);
        assert!(!status.network_initialized);
        assert_eq!(status.open_mode, CloudsyncOpenMode::Disabled);

        db.cloudsync_stop().await.unwrap();
    }

    #[tokio::test]
    async fn enabled_open_mode_requires_runtime_config_before_start() {
        let db = Db3::connect_memory().await.unwrap();

        let error = db.cloudsync_start().await.unwrap_err();
        assert!(matches!(error, CloudsyncRuntimeError::NotConfigured));
    }

    #[tokio::test]
    async fn emits_table_changes_for_local_writes() {
        let db = Db3::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool().as_ref())
            .await
            .unwrap();

        let mut changes = db.subscribe_table_changes();
        let before = db.pool().current_table_change_seq();

        sqlx::query("INSERT INTO test_events (id) VALUES ('a')")
            .execute(db.pool().as_ref())
            .await
            .unwrap();

        let change = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(change.table, "test_events");
        assert_eq!(change.kind, TableChangeKind::Insert);
        assert!(change.seq > before);
        assert_eq!(db.pool().current_table_change_seq(), change.seq);
        assert_eq!(
            db.pool().latest_table_change_seq("test_events"),
            Some(change.seq)
        );
    }

    #[tokio::test]
    async fn emits_table_changes_only_after_commit() {
        let db = Db3::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool().as_ref())
            .await
            .unwrap();

        let mut changes = db.subscribe_table_changes();
        let mut tx = db.pool().begin().await.unwrap();

        sqlx::query("INSERT INTO test_events (id) VALUES ('a')")
            .execute(&mut *tx)
            .await
            .unwrap();

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), changes.recv())
                .await
                .is_err()
        );

        tx.commit().await.unwrap();

        let change = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(change.table, "test_events");
        assert_eq!(change.kind, TableChangeKind::Insert);
        assert_eq!(
            db.pool().latest_table_change_seq("test_events"),
            Some(change.seq)
        );
    }

    #[tokio::test]
    async fn rollback_clears_pending_table_changes() {
        let db = Db3::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool().as_ref())
            .await
            .unwrap();

        let mut changes = db.subscribe_table_changes();
        let mut tx = db.pool().begin().await.unwrap();

        sqlx::query("INSERT INTO test_events (id) VALUES ('a')")
            .execute(&mut *tx)
            .await
            .unwrap();

        tx.rollback().await.unwrap();

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), changes.recv())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn coalesces_multiple_writes_in_a_transaction() {
        let db = Db3::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL)")
            .execute(db.pool().as_ref())
            .await
            .unwrap();

        let mut changes = db.subscribe_table_changes();
        let mut tx = db.pool().begin().await.unwrap();

        sqlx::query("INSERT INTO test_events (id, value) VALUES ('a', 'before')")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("UPDATE test_events SET value = 'after' WHERE id = 'a'")
            .execute(&mut *tx)
            .await
            .unwrap();

        tx.commit().await.unwrap();

        let change = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(change.table, "test_events");
        assert_eq!(change.kind, TableChangeKind::Update);
        assert_eq!(
            db.pool().latest_table_change_seq("test_events"),
            Some(change.seq)
        );
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), changes.recv())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn emits_update_and_delete_table_changes() {
        let db = Db3::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL)")
            .execute(db.pool().as_ref())
            .await
            .unwrap();
        sqlx::query("INSERT INTO test_events (id, value) VALUES ('a', 'before')")
            .execute(db.pool().as_ref())
            .await
            .unwrap();

        let mut changes = db.subscribe_table_changes();

        sqlx::query("UPDATE test_events SET value = 'after' WHERE id = 'a'")
            .execute(db.pool().as_ref())
            .await
            .unwrap();
        sqlx::query("DELETE FROM test_events WHERE id = 'a'")
            .execute(db.pool().as_ref())
            .await
            .unwrap();

        let update = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();
        let delete = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(update.table, "test_events");
        assert_eq!(update.kind, TableChangeKind::Update);
        assert_eq!(delete.table, "test_events");
        assert_eq!(delete.kind, TableChangeKind::Delete);
        assert!(delete.seq > update.seq);
        assert_eq!(
            db.pool().latest_table_change_seq("test_events"),
            Some(delete.seq)
        );
    }

    #[tokio::test]
    async fn emits_table_changes_across_multiple_connections() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.db");

        let db = Db3::open_with_migrate(
            DbOpenOptions {
                storage: DbStorage::Local(&path),
                cloudsync_open_mode: CloudsyncOpenMode::Disabled,
                journal_mode_wal: true,
                foreign_keys: true,
                max_connections: Some(4),
                migration_failure_policy: MigrationFailurePolicy::Fail,
            },
            |pool| {
                Box::pin(async move {
                    sqlx::query("CREATE TABLE multi_conn_events (id TEXT PRIMARY KEY NOT NULL)")
                        .execute(pool)
                        .await
                        .unwrap();
                    Ok::<(), sqlx::Error>(())
                })
            },
        )
        .await
        .unwrap();

        let mut changes = db.subscribe_table_changes();
        let mut conn_a = db.pool().acquire().await.unwrap();
        let mut conn_b = db.pool().acquire().await.unwrap();

        sqlx::query("INSERT INTO multi_conn_events (id) VALUES ('a')")
            .execute(&mut *conn_a)
            .await
            .unwrap();
        sqlx::query("INSERT INTO multi_conn_events (id) VALUES ('b')")
            .execute(&mut *conn_b)
            .await
            .unwrap();

        let first = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();
        let second = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(first.table, "multi_conn_events");
        assert_eq!(second.table, "multi_conn_events");
        assert_ne!(first.seq, second.seq);
    }

    #[tokio::test]
    async fn tracks_monotonic_change_sequences_per_table() {
        let db = Db3::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool().as_ref())
            .await
            .unwrap();
        sqlx::query("CREATE TABLE other_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool().as_ref())
            .await
            .unwrap();

        let start = db.pool().current_table_change_seq();
        let mut changes = db.subscribe_table_changes();

        sqlx::query("INSERT INTO test_events (id) VALUES ('a')")
            .execute(db.pool().as_ref())
            .await
            .unwrap();
        let first = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        sqlx::query("INSERT INTO test_events (id) VALUES ('b')")
            .execute(db.pool().as_ref())
            .await
            .unwrap();
        let second = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        sqlx::query("INSERT INTO other_events (id) VALUES ('c')")
            .execute(db.pool().as_ref())
            .await
            .unwrap();
        let third = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(first.seq > start);
        assert!(second.seq > first.seq);
        assert!(third.seq > second.seq);
        assert_eq!(db.pool().current_table_change_seq(), third.seq);
        assert_eq!(
            db.pool().latest_table_change_seq("test_events"),
            Some(second.seq)
        );
        assert_eq!(
            db.pool().latest_table_change_seq("other_events"),
            Some(third.seq)
        );
        assert_eq!(db.pool().latest_table_change_seq("missing_events"), None);
    }

    #[tokio::test]
    async fn open_with_migrate_memory_clamps_max_connections_to_one() {
        let db = Db3::open_with_migrate(
            DbOpenOptions {
                storage: DbStorage::Memory,
                cloudsync_open_mode: CloudsyncOpenMode::Disabled,
                journal_mode_wal: false,
                foreign_keys: true,
                max_connections: Some(4),
                migration_failure_policy: MigrationFailurePolicy::Fail,
            },
            |_pool| Box::pin(async { Ok::<(), sqlx::Error>(()) }),
        )
        .await
        .unwrap();

        let _conn = db.pool().acquire().await.unwrap();

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), db.pool().acquire())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn cloned_pool_keeps_hooks_alive_after_db_drop() {
        let db = Db3::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE TABLE retained_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool().as_ref())
            .await
            .unwrap();

        let pool = db.pool().clone();
        let mut changes = pool.subscribe_table_changes();
        drop(db);

        sqlx::query("INSERT INTO retained_events (id) VALUES ('a')")
            .execute(pool.as_ref())
            .await
            .unwrap();

        let change = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(change.table, "retained_events");
        assert_eq!(change.kind, TableChangeKind::Insert);
        assert_eq!(
            pool.latest_table_change_seq("retained_events"),
            Some(change.seq)
        );
    }
}

mod cloudsync;
mod pool;

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use hypr_cloudsync::Error;
use sqlx::sqlite::SqliteConnectOptions;

use crate::cloudsync::CloudsyncRuntimeState;
pub use crate::cloudsync::{
    CloudsyncAuth, CloudsyncRuntimeConfig, CloudsyncRuntimeError, CloudsyncStatus,
    CloudsyncTableSpec, cloudsync_begin_alter_on, cloudsync_commit_alter_on,
};
use crate::pool::connect_pool;
pub use crate::pool::{DbPool, TableChange, TableChangeKind};

#[derive(Clone, Copy, Debug)]
pub enum DbStorage<'a> {
    Local(&'a Path),
    Memory,
}

#[derive(Clone, Copy, Debug)]
pub struct DbOpenOptions<'a> {
    pub storage: DbStorage<'a>,
    pub cloudsync_enabled: bool,
    pub journal_mode_wal: bool,
    pub foreign_keys: bool,
    pub max_connections: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum DbOpenError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Cloudsync(#[from] hypr_cloudsync::Error),
}

pub type ManagedDb = std::sync::Arc<Db3>;

const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Db3 {
    pub(crate) cloudsync_enabled: bool,
    pub(crate) cloudsync_path: Option<PathBuf>,
    pub(crate) cloudsync_runtime: Arc<Mutex<CloudsyncRuntimeState>>,
    pub(crate) pool: DbPool,
}

impl std::fmt::Debug for Db3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let runtime = self.cloudsync_runtime.lock().unwrap();
        f.debug_struct("Db3")
            .field("cloudsync_enabled", &self.cloudsync_enabled)
            .field("cloudsync_path", &self.cloudsync_path)
            .field("cloudsync_runtime", &*runtime)
            .finish_non_exhaustive()
    }
}

impl Drop for Db3 {
    fn drop(&mut self) {
        let task = {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.running = false;
            runtime.task.take()
        };

        if let Some(mut task) = task {
            if let Some(shutdown_tx) = task.shutdown_tx.take() {
                let _ = shutdown_tx.send(());
            }
            task.join_handle.abort();
        }
    }
}

impl Db3 {
    pub async fn open(options: DbOpenOptions<'_>) -> Result<Self, DbOpenError> {
        connect_with_options(&options).await
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
            cloudsync_enabled: true,
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
            cloudsync_enabled: true,
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
            cloudsync_enabled: false,
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
            cloudsync_enabled: false,
            cloudsync_path: None,
            cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
            pool,
        })
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }
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
        if options.cloudsync_enabled {
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
        cloudsync_enabled: options.cloudsync_enabled,
        cloudsync_path,
        cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
        pool,
    })
}

fn apply_internal_connect_policy(connect_options: SqliteConnectOptions) -> SqliteConnectOptions {
    connect_options.busy_timeout(SQLITE_BUSY_TIMEOUT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use tokio::sync::oneshot;

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
    async fn open_applies_requested_pragmas() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("app.db");

        let db = Db3::open(DbOpenOptions {
            storage: DbStorage::Local(&db_path),
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
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
        let db = Db3::open(DbOpenOptions {
            storage: DbStorage::Memory,
            cloudsync_enabled: false,
            journal_mode_wal: false,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();

        assert!(!db.cloudsync_enabled());
        assert!(!db.has_cloudsync());

        db.cloudsync_configure(test_cloudsync_config()).unwrap();
        db.cloudsync_start().await.unwrap();

        let status = db.cloudsync_status().await.unwrap();
        assert!(status.configured);
        assert!(!status.extension_loaded);
        assert!(!status.running);
        assert!(!status.network_initialized);
        assert!(!status.cloudsync_enabled);

        db.cloudsync_stop().await.unwrap();
    }

    #[tokio::test]
    async fn enabled_open_mode_requires_runtime_config_before_start() {
        let db = Db3::connect_memory().await.unwrap();

        let error = db.cloudsync_start().await.unwrap_err();
        assert!(matches!(error, CloudsyncRuntimeError::NotConfigured));
    }

    #[tokio::test]
    async fn configure_rejects_live_runtime_changes() {
        let db = Db3::connect_memory_plain().await.unwrap();
        db.cloudsync_configure(test_cloudsync_config()).unwrap();
        db.cloudsync_runtime.lock().unwrap().running = true;

        let error = db
            .cloudsync_configure(CloudsyncRuntimeConfig {
                connection_string: "sqlitecloud://demo.invalid/other.db?apikey=demo".to_string(),
                ..test_cloudsync_config()
            })
            .unwrap_err();

        assert!(matches!(error, CloudsyncRuntimeError::RestartRequired));
        assert_eq!(
            db.cloudsync_runtime
                .lock()
                .unwrap()
                .config
                .as_ref()
                .unwrap()
                .connection_string,
            "sqlitecloud://demo.invalid/app.db?apikey=demo"
        );
    }

    #[tokio::test]
    async fn reconfigure_preserves_stopped_state_when_runtime_is_inert() {
        let db = Db3::open(DbOpenOptions {
            storage: DbStorage::Memory,
            cloudsync_enabled: false,
            journal_mode_wal: false,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        db.cloudsync_configure(test_cloudsync_config()).unwrap();
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.running = true;
            runtime.network_initialized = true;
        }

        let next_config = CloudsyncRuntimeConfig {
            connection_string: "sqlitecloud://demo.invalid/reconfigured.db?apikey=demo".to_string(),
            sync_interval_ms: 2_000,
            ..test_cloudsync_config()
        };

        db.cloudsync_reconfigure(next_config.clone()).await.unwrap();

        let runtime = db.cloudsync_runtime.lock().unwrap();
        assert_eq!(runtime.config, Some(next_config));
        assert!(!runtime.running);
        assert!(!runtime.network_initialized);
    }

    #[tokio::test]
    async fn dropping_db_stops_background_task_best_effort() {
        struct DropFlag(Arc<AtomicBool>);

        impl Drop for DropFlag {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let db = Db3::connect_memory_plain().await.unwrap();
        let dropped = Arc::new(AtomicBool::new(false));
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let guard = DropFlag(Arc::clone(&dropped));
        let join_handle = tokio::spawn(async move {
            let _guard = guard;
            let _ = shutdown_rx.await;
        });

        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.running = true;
            runtime.task = Some(crate::cloudsync::CloudsyncBackgroundTask {
                shutdown_tx: Some(shutdown_tx),
                join_handle,
            });
        }

        drop(db);

        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !dropped.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
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

        let db = Db3::open(DbOpenOptions {
            storage: DbStorage::Local(&path),
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(4),
        })
        .await
        .unwrap();
        sqlx::query("CREATE TABLE multi_conn_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool().as_ref())
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
    async fn open_memory_clamps_max_connections_to_one() {
        let db = Db3::open(DbOpenOptions {
            storage: DbStorage::Memory,
            cloudsync_enabled: false,
            journal_mode_wal: false,
            foreign_keys: true,
            max_connections: Some(4),
        })
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

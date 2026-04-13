use std::ffi::{CStr, c_void};
use std::future::Future;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;

pub use hypr_cloudsync::Error;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tokio::sync::broadcast;

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
    pub cloudsync: bool,
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

#[derive(Debug)]
pub struct Db3 {
    cloudsync_path: Option<PathBuf>,
    pool: DbPool,
}

#[derive(Clone, Debug)]
pub struct DbPool {
    pool: SqlitePool,
    table_change_tx: Arc<broadcast::Sender<TableChange>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TableChangeKind {
    Insert,
    Update,
    Delete,
}

/// Best-effort table-level mutation signal emitted for writes observed on pooled SQLite
/// connections created by this crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableChange {
    pub table: String,
    pub kind: TableChangeKind,
}

type BoxedMigrationFuture<'a, E> = Pin<Box<dyn Future<Output = Result<(), E>> + Send + 'a>>;

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
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let (options, cloudsync_path) = hypr_cloudsync::apply(options)?;
        let pool = connect_pool(options, None).await.map_err(Error::from)?;

        Ok(Self {
            cloudsync_path: Some(cloudsync_path),
            pool,
        })
    }

    pub async fn connect_memory() -> Result<Self, Error> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let (options, cloudsync_path) = hypr_cloudsync::apply(options)?;
        let pool = connect_pool(options, Some(1)).await.map_err(Error::from)?;

        Ok(Self {
            cloudsync_path: Some(cloudsync_path),
            pool,
        })
    }

    pub async fn connect_local_plain(path: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(sqlx::Error::Io)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .pragma("foreign_keys", "ON");
        let pool = connect_pool(options, None).await?;

        Ok(Self {
            cloudsync_path: None,
            pool,
        })
    }

    pub async fn connect_memory_plain() -> Result<Self, sqlx::Error> {
        let options =
            SqliteConnectOptions::from_str("sqlite::memory:")?.pragma("foreign_keys", "ON");
        let pool = connect_pool(options, Some(1)).await?;

        Ok(Self {
            cloudsync_path: None,
            pool,
        })
    }

    pub fn has_cloudsync(&self) -> bool {
        self.cloudsync_path.is_some()
    }

    pub fn cloudsync_path(&self) -> Option<&Path> {
        self.cloudsync_path.as_deref()
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    /// Subscribe to best-effort table-level change notifications for writes observed through
    /// this app's pooled SQLite connections.
    pub fn subscribe_table_changes(&self) -> broadcast::Receiver<TableChange> {
        self.pool.subscribe_table_changes()
    }

    pub async fn cloudsync_version(&self) -> Result<String, Error> {
        hypr_cloudsync::version(self.pool.as_ref()).await
    }

    pub async fn cloudsync_init(
        &self,
        table_name: &str,
        crdt_algo: Option<&str>,
        force: Option<bool>,
    ) -> Result<(), Error> {
        hypr_cloudsync::init(self.pool.as_ref(), table_name, crdt_algo, force).await
    }

    pub async fn cloudsync_network_init(&self, connection_string: &str) -> Result<(), Error> {
        hypr_cloudsync::network_init(self.pool.as_ref(), connection_string).await
    }

    pub async fn cloudsync_network_set_apikey(&self, api_key: &str) -> Result<(), Error> {
        hypr_cloudsync::network_set_apikey(self.pool.as_ref(), api_key).await
    }

    pub async fn cloudsync_network_set_token(&self, token: &str) -> Result<(), Error> {
        hypr_cloudsync::network_set_token(self.pool.as_ref(), token).await
    }

    pub async fn cloudsync_network_sync(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<(), Error> {
        hypr_cloudsync::network_sync(self.pool.as_ref(), wait_ms, max_retries).await
    }
}

impl DbPool {
    /// Subscribe to best-effort table-level change notifications for writes observed through
    /// this pool's physical SQLite connections.
    pub fn subscribe_table_changes(&self) -> broadcast::Receiver<TableChange> {
        self.table_change_tx.subscribe()
    }

    pub async fn close(self) {
        self.pool.close().await;
    }
}

impl AsRef<SqlitePool> for DbPool {
    fn as_ref(&self) -> &SqlitePool {
        &self.pool
    }
}

impl Deref for DbPool {
    type Target = SqlitePool;

    fn deref(&self) -> &Self::Target {
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
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
        }
        DbStorage::Memory => SqliteConnectOptions::from_str("sqlite::memory:")?,
    };

    if options.journal_mode_wal {
        connect_options = connect_options.pragma("journal_mode", "WAL");
    }
    if options.foreign_keys {
        connect_options = connect_options.pragma("foreign_keys", "ON");
    }

    let (connect_options, cloudsync_path) = if options.cloudsync {
        let (connect_options, cloudsync_path) = hypr_cloudsync::apply(connect_options)?;
        (connect_options, Some(cloudsync_path))
    } else {
        (connect_options, None)
    };

    let pool = connect_pool(connect_options, options.max_connections).await?;

    Ok(Db3 {
        cloudsync_path,
        pool,
    })
}

async fn connect_pool(
    connect_options: SqliteConnectOptions,
    max_connections: Option<u32>,
) -> Result<DbPool, sqlx::Error> {
    let (table_change_tx, _) = broadcast::channel(256);
    let table_change_tx = Arc::new(table_change_tx);

    let callback_tx = Arc::clone(&table_change_tx);
    let mut pool_options = SqlitePoolOptions::new().after_connect(move |conn, _| {
        let callback_tx = Arc::clone(&callback_tx);
        Box::pin(async move {
            let mut handle = conn.lock_handle().await?;
            let raw = handle.as_raw_handle().as_ptr();

            unsafe {
                libsqlite3_sys::sqlite3_update_hook(
                    raw,
                    Some(update_hook_callback),
                    Arc::as_ptr(&callback_tx) as *mut c_void,
                );
            }

            Ok(())
        })
    });

    if let Some(max_connections) = max_connections {
        pool_options = pool_options.max_connections(max_connections);
    }

    let pool = pool_options.connect_with(connect_options).await?;
    Ok(DbPool {
        pool,
        table_change_tx,
    })
}

unsafe extern "C" fn update_hook_callback(
    user_data: *mut c_void,
    op: std::os::raw::c_int,
    _db_name: *const std::os::raw::c_char,
    table_name: *const std::os::raw::c_char,
    _row_id: libsqlite3_sys::sqlite3_int64,
) {
    let kind = match op {
        libsqlite3_sys::SQLITE_INSERT => TableChangeKind::Insert,
        libsqlite3_sys::SQLITE_UPDATE => TableChangeKind::Update,
        libsqlite3_sys::SQLITE_DELETE => TableChangeKind::Delete,
        _ => return,
    };

    let table = unsafe { CStr::from_ptr(table_name) }
        .to_string_lossy()
        .into_owned();
    let tx = unsafe { &*(user_data as *const broadcast::Sender<TableChange>) };
    let _ = tx.send(TableChange { table, kind });
}

fn recreate_storage(options: &DbOpenOptions<'_>) -> Result<(), DbOpenError> {
    match options.storage {
        DbStorage::Local(path) => {
            wipe_db_file(path);
            if options.cloudsync {
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
                cloudsync: false,
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
                cloudsync: false,
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
                cloudsync: false,
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
                cloudsync: false,
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

        assert_eq!(foreign_keys, 1);
        assert_eq!(journal_mode.to_lowercase(), "wal");
    }

    #[tokio::test]
    async fn emits_table_changes_for_local_writes() {
        let db = Db3::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool().as_ref())
            .await
            .unwrap();

        let mut changes = db.subscribe_table_changes();

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
    }

    #[tokio::test]
    async fn emits_table_changes_across_multiple_connections() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.db");

        let db = Db3::open_with_migrate(
            DbOpenOptions {
                storage: DbStorage::Local(&path),
                cloudsync: false,
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
    }
}

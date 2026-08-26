mod cloudsync;

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sqlx::pool::PoolConnection;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Connection, Sqlite, SqlitePool};

pub use crate::cloudsync::{
    CLOUDSYNC_MAX_OUTBOUND_BYTES, CLOUDSYNC_MAX_OUTBOUND_CHUNKS, CLOUDSYNC_MAX_OUTBOUND_ROWS,
    CloudsyncActivityEntry, CloudsyncActivityStatus, CloudsyncActivityTrigger, CloudsyncAuth,
    CloudsyncBeforeHookFuture, CloudsyncHookFuture, CloudsyncHookOutcome, CloudsyncNetworkResult,
    CloudsyncRuntimeConfig, CloudsyncRuntimeError, CloudsyncStatus, CloudsyncSyncDirective,
    CloudsyncSyncHook, CloudsyncTableSpec, cloudsync_begin_alter_on, cloudsync_commit_alter_on,
    cloudsync_is_enabled_on,
};
use crate::cloudsync::{CloudsyncInterruptHandle, CloudsyncRuntimeState};

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
    Cloudsync(#[from] anlg_cloudsync::Error),
}

pub type ManagedDb = std::sync::Arc<Db>;

const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Db {
    pub(crate) cloudsync_enabled: bool,
    pub(crate) cloudsync_path: Option<PathBuf>,
    pub(crate) cloudsync_initializer: anlg_cloudsync::CloudsyncConnectionInitializer,
    pub(crate) cloudsync_connection: Arc<tokio::sync::Mutex<Option<PoolConnection<Sqlite>>>>,
    pub(crate) cloudsync_interrupt: Arc<CloudsyncInterruptHandle>,
    pub(crate) cloudsync_lifecycle: Arc<tokio::sync::Mutex<()>>,
    pub(crate) cloudsync_sync_operation: Arc<tokio::sync::Mutex<()>>,
    pub(crate) cloudsync_sync_requested: Arc<tokio::sync::Notify>,
    pub(crate) cloudsync_runtime: Arc<Mutex<CloudsyncRuntimeState>>,
    pub(crate) cloudsync_sync_hook: Arc<Mutex<Option<Arc<dyn CloudsyncSyncHook>>>>,
    pub(crate) pool: SqlitePool,
    change_notifier: anlg_db_change::ChangeNotifier,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let runtime = self.cloudsync_runtime.lock().unwrap();
        f.debug_struct("Db")
            .field("cloudsync_enabled", &self.cloudsync_enabled)
            .field("cloudsync_path", &self.cloudsync_path)
            .field("cloudsync_runtime", &*runtime)
            .field("change_notifier", &true)
            .finish_non_exhaustive()
    }
}

impl Drop for Db {
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

impl Db {
    pub async fn open(options: DbOpenOptions<'_>) -> Result<Self, DbOpenError> {
        if options.cloudsync_enabled
            && matches!(options.storage, DbStorage::Local(_))
            && !options.journal_mode_wal
        {
            return Err(anlg_cloudsync::Error::WalRequired.into());
        }

        let cloudsync_initializer = anlg_cloudsync::CloudsyncConnectionInitializer::default();
        let (change_notifier, pool_options) = match (options.cloudsync_enabled, options.storage) {
            (true, DbStorage::Local(_)) => {
                anlg_db_change::ChangeNotifier::new_with_cloudsync(cloudsync_initializer.clone())
            }
            (true, DbStorage::Memory) => anlg_db_change::ChangeNotifier::disabled(),
            (false, _) => anlg_db_change::ChangeNotifier::new(),
        };
        connect_with_options(
            &options,
            pool_options,
            change_notifier,
            cloudsync_initializer,
        )
        .await
    }

    pub fn change_notifier(&self) -> &anlg_db_change::ChangeNotifier {
        &self.change_notifier
    }

    pub async fn connect_local(path: impl AsRef<Path>) -> Result<Self, anlg_cloudsync::Error> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let options = apply_internal_connect_policy(SqliteConnectOptions::new())
            .filename(path)
            .create_if_missing(true)
            .pragma("journal_mode", "WAL");
        let cloudsync_initializer = anlg_cloudsync::CloudsyncConnectionInitializer::default();
        let (options, cloudsync_path) =
            anlg_cloudsync::apply_with_initializer(options, &cloudsync_initializer)?;
        let (change_notifier, pool_options) =
            anlg_db_change::ChangeNotifier::new_with_cloudsync(cloudsync_initializer.clone());
        let pool = apply_internal_pool_policy(pool_options)
            .connect_with(options)
            .await
            .map_err(anlg_cloudsync::Error::from)?;
        ensure_cloudsync_wal(&pool).await?;

        Ok(Self {
            cloudsync_enabled: true,
            cloudsync_path: Some(cloudsync_path),
            cloudsync_initializer,
            cloudsync_connection: Arc::new(tokio::sync::Mutex::new(None)),
            cloudsync_interrupt: Arc::new(CloudsyncInterruptHandle::default()),
            cloudsync_lifecycle: Arc::new(tokio::sync::Mutex::new(())),
            cloudsync_sync_operation: Arc::new(tokio::sync::Mutex::new(())),
            cloudsync_sync_requested: Arc::new(tokio::sync::Notify::new()),
            cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
            cloudsync_sync_hook: Arc::new(Mutex::new(None)),
            pool,
            change_notifier,
        })
    }

    pub async fn connect_memory() -> Result<Self, anlg_cloudsync::Error> {
        let options =
            apply_internal_connect_policy(SqliteConnectOptions::from_str("sqlite::memory:")?);
        let (options, cloudsync_path) = anlg_cloudsync::apply(options)?;
        let (change_notifier, pool_options) = anlg_db_change::ChangeNotifier::disabled();
        let pool = apply_internal_pool_policy(pool_options)
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(anlg_cloudsync::Error::from)?;

        Ok(Self {
            cloudsync_enabled: true,
            cloudsync_path: Some(cloudsync_path),
            cloudsync_initializer: anlg_cloudsync::CloudsyncConnectionInitializer::default(),
            cloudsync_connection: Arc::new(tokio::sync::Mutex::new(None)),
            cloudsync_interrupt: Arc::new(CloudsyncInterruptHandle::default()),
            cloudsync_lifecycle: Arc::new(tokio::sync::Mutex::new(())),
            cloudsync_sync_operation: Arc::new(tokio::sync::Mutex::new(())),
            cloudsync_sync_requested: Arc::new(tokio::sync::Notify::new()),
            cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
            cloudsync_sync_hook: Arc::new(Mutex::new(None)),
            pool,
            change_notifier,
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
        let (change_notifier, pool_options) = anlg_db_change::ChangeNotifier::new();
        let pool = apply_internal_pool_policy(pool_options)
            .connect_with(options)
            .await?;

        Ok(Self {
            cloudsync_enabled: false,
            cloudsync_path: None,
            cloudsync_initializer: anlg_cloudsync::CloudsyncConnectionInitializer::default(),
            cloudsync_connection: Arc::new(tokio::sync::Mutex::new(None)),
            cloudsync_interrupt: Arc::new(CloudsyncInterruptHandle::default()),
            cloudsync_lifecycle: Arc::new(tokio::sync::Mutex::new(())),
            cloudsync_sync_operation: Arc::new(tokio::sync::Mutex::new(())),
            cloudsync_sync_requested: Arc::new(tokio::sync::Notify::new()),
            cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
            cloudsync_sync_hook: Arc::new(Mutex::new(None)),
            pool,
            change_notifier,
        })
    }

    pub async fn connect_local_read_write(path: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let path = path.as_ref();
        if !path.is_file() {
            return Err(sqlx::Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("database file not found: {}", path.display()),
            )));
        }

        let options = apply_internal_connect_policy(SqliteConnectOptions::new())
            .filename(path)
            .create_if_missing(false)
            .pragma("foreign_keys", "ON")
            .pragma("journal_mode", "WAL");
        let (change_notifier, pool_options) = anlg_db_change::ChangeNotifier::new();
        let pool = apply_internal_pool_policy(pool_options)
            .connect_with(options)
            .await?;

        Ok(Self {
            cloudsync_enabled: false,
            cloudsync_path: None,
            cloudsync_initializer: anlg_cloudsync::CloudsyncConnectionInitializer::default(),
            cloudsync_connection: Arc::new(tokio::sync::Mutex::new(None)),
            cloudsync_interrupt: Arc::new(CloudsyncInterruptHandle::default()),
            cloudsync_lifecycle: Arc::new(tokio::sync::Mutex::new(())),
            cloudsync_sync_operation: Arc::new(tokio::sync::Mutex::new(())),
            cloudsync_sync_requested: Arc::new(tokio::sync::Notify::new()),
            cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
            cloudsync_sync_hook: Arc::new(Mutex::new(None)),
            pool,
            change_notifier,
        })
    }

    pub async fn connect_local_read_only(path: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let options = apply_internal_connect_policy(SqliteConnectOptions::new())
            .filename(path)
            .read_only(true)
            .pragma("foreign_keys", "ON")
            .pragma("query_only", "ON");
        let (change_notifier, pool_options) = anlg_db_change::ChangeNotifier::new();
        let pool = apply_internal_pool_policy(pool_options)
            .connect_with(options)
            .await?;

        Ok(Self {
            cloudsync_enabled: false,
            cloudsync_path: None,
            cloudsync_initializer: anlg_cloudsync::CloudsyncConnectionInitializer::default(),
            cloudsync_connection: Arc::new(tokio::sync::Mutex::new(None)),
            cloudsync_interrupt: Arc::new(CloudsyncInterruptHandle::default()),
            cloudsync_lifecycle: Arc::new(tokio::sync::Mutex::new(())),
            cloudsync_sync_operation: Arc::new(tokio::sync::Mutex::new(())),
            cloudsync_sync_requested: Arc::new(tokio::sync::Notify::new()),
            cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
            cloudsync_sync_hook: Arc::new(Mutex::new(None)),
            pool,
            change_notifier,
        })
    }

    pub async fn connect_memory_plain() -> Result<Self, sqlx::Error> {
        let options =
            apply_internal_connect_policy(SqliteConnectOptions::from_str("sqlite::memory:")?)
                .pragma("foreign_keys", "ON");
        let (change_notifier, pool_options) = anlg_db_change::ChangeNotifier::new();
        let pool = apply_internal_pool_policy(pool_options)
            .max_connections(1)
            .connect_with(options)
            .await?;

        Ok(Self {
            cloudsync_enabled: false,
            cloudsync_path: None,
            cloudsync_initializer: anlg_cloudsync::CloudsyncConnectionInitializer::default(),
            cloudsync_connection: Arc::new(tokio::sync::Mutex::new(None)),
            cloudsync_interrupt: Arc::new(CloudsyncInterruptHandle::default()),
            cloudsync_lifecycle: Arc::new(tokio::sync::Mutex::new(())),
            cloudsync_sync_operation: Arc::new(tokio::sync::Mutex::new(())),
            cloudsync_sync_requested: Arc::new(tokio::sync::Notify::new()),
            cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
            cloudsync_sync_hook: Arc::new(Mutex::new(None)),
            pool,
            change_notifier,
        })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn set_cloudsync_sync_hook(&self, hook: Arc<dyn CloudsyncSyncHook>) {
        self.cloudsync_sync_hook.lock().unwrap().replace(hook);
    }
}

async fn connect_with_options(
    options: &DbOpenOptions<'_>,
    pool_options: SqlitePoolOptions,
    change_notifier: anlg_db_change::ChangeNotifier,
    cloudsync_initializer: anlg_cloudsync::CloudsyncConnectionInitializer,
) -> Result<Db, DbOpenError> {
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

    let (connect_options, cloudsync_path) = match (options.cloudsync_enabled, options.storage) {
        (true, DbStorage::Local(_)) => {
            let (connect_options, cloudsync_path) =
                anlg_cloudsync::apply_with_initializer(connect_options, &cloudsync_initializer)?;
            (connect_options, Some(cloudsync_path))
        }
        (true, DbStorage::Memory) => {
            let (connect_options, cloudsync_path) = anlg_cloudsync::apply(connect_options)?;
            (connect_options, Some(cloudsync_path))
        }
        (false, _) => (connect_options, None),
    };

    let mut pool_options = apply_internal_pool_policy(pool_options);
    match options.storage {
        DbStorage::Memory => {
            pool_options = pool_options.max_connections(1);
        }
        DbStorage::Local(_) => {
            if let Some(max) = options.max_connections {
                pool_options = pool_options.max_connections(max);
            }
        }
    };
    let pool = pool_options.connect_with(connect_options).await?;
    if options.cloudsync_enabled && matches!(options.storage, DbStorage::Local(_)) {
        ensure_cloudsync_wal(&pool).await?;
    }

    Ok(Db {
        cloudsync_enabled: options.cloudsync_enabled,
        cloudsync_path,
        cloudsync_initializer,
        cloudsync_connection: Arc::new(tokio::sync::Mutex::new(None)),
        cloudsync_interrupt: Arc::new(CloudsyncInterruptHandle::default()),
        cloudsync_lifecycle: Arc::new(tokio::sync::Mutex::new(())),
        cloudsync_sync_operation: Arc::new(tokio::sync::Mutex::new(())),
        cloudsync_sync_requested: Arc::new(tokio::sync::Notify::new()),
        cloudsync_runtime: Arc::new(Mutex::new(CloudsyncRuntimeState::default())),
        cloudsync_sync_hook: Arc::new(Mutex::new(None)),
        pool,
        change_notifier,
    })
}

fn apply_internal_connect_policy(connect_options: SqliteConnectOptions) -> SqliteConnectOptions {
    connect_options.busy_timeout(SQLITE_BUSY_TIMEOUT)
}

fn apply_internal_pool_policy(pool_options: SqlitePoolOptions) -> SqlitePoolOptions {
    pool_options.after_release(|connection, _| {
        Box::pin(async move {
            if !connection.is_in_transaction() {
                return Ok(true);
            }

            tracing::warn!("sqlite_connection_returned_in_transaction");
            if let Err(error) = connection.ping().await {
                tracing::error!(%error, "sqlite_transaction_repair_failed");
                return Ok(false);
            }

            if connection.is_in_transaction() {
                tracing::error!("sqlite_connection_rejected_in_transaction");
                return Ok(false);
            }

            tracing::info!("sqlite_transaction_repaired_before_pool_return");
            Ok(true)
        })
    })
}

async fn ensure_cloudsync_wal(pool: &SqlitePool) -> Result<(), anlg_cloudsync::Error> {
    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(pool)
        .await?;
    if journal_mode.eq_ignore_ascii_case("wal") {
        Ok(())
    } else {
        Err(anlg_cloudsync::Error::WalRequired)
    }
}

#[cfg(test)]
mod tests;

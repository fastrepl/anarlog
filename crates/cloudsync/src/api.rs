use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock, Weak};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::{Executor, Sqlite, SqliteConnection};

use crate::error::Error;

// PRAGMA busy_timeout = 50 keeps SQLite's C busy-handler short so after_connect
// does not park a Tokio worker. Retry SQLITE_BUSY asynchronously for the same
// 5s budget used by the rest of the app instead of failing the pool connect.
const EXTENSION_LOAD_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
type ExtensionLoadLock = tokio::sync::Mutex<()>;

static EXTENSION_LOAD_LOCKS: OnceLock<std::sync::Mutex<HashMap<PathBuf, Weak<ExtensionLoadLock>>>> =
    OnceLock::new();

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CloudsyncTableSpec {
    pub table_name: String,
    pub crdt_algo: Option<String>,
    pub init_flags: Option<i64>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CloudsyncConnectionInitializer {
    tables: Arc<RwLock<Vec<CloudsyncTableSpec>>>,
    extension_path: Arc<RwLock<Option<PathBuf>>>,
    extension_load_lock: Arc<RwLock<Option<Arc<ExtensionLoadLock>>>>,
}

impl CloudsyncConnectionInitializer {
    pub fn set_extension_path(&self, extension_path: PathBuf, database_path: PathBuf) {
        *self.extension_path.write().unwrap() = Some(extension_path);
        *self.extension_load_lock.write().unwrap() = Some(extension_load_lock_for(database_path));
    }

    pub fn replace_tables(&self, tables: Vec<CloudsyncTableSpec>) {
        *self.tables.write().unwrap() = tables;
    }

    pub fn clear(&self) {
        self.tables.write().unwrap().clear();
    }

    pub async fn initialize(&self, connection: &mut SqliteConnection) -> Result<(), Error> {
        let extension_path = self.extension_path.read().unwrap().clone();
        let extension_load_lock = self.extension_load_lock.read().unwrap().clone();
        let _extension_load = match (&extension_path, extension_load_lock.as_ref()) {
            (Some(_), Some(lock)) => Some(lock.lock().await),
            (Some(_), None) => {
                return Err(Error::ExtensionInitialization(
                    "sqlite-sync extension load lock is not configured".to_string(),
                ));
            }
            (None, _) => None,
        };
        if let Some(extension_path) = extension_path.as_deref() {
            load_and_validate_extension(connection, extension_path).await?;
        }

        let tables = self.tables.read().unwrap().clone();
        for table in tables.iter().filter(|table| table.enabled) {
            if !is_enabled(&mut *connection, &table.table_name).await? {
                init(
                    &mut *connection,
                    &table.table_name,
                    table.crdt_algo.as_deref(),
                    table.init_flags,
                )
                .await?;
            }
            if !is_enabled(&mut *connection, &table.table_name).await? {
                return Err(Error::ExtensionInitialization(format!(
                    "sqlite-sync did not initialize table '{}'",
                    table.table_name
                )));
            }
        }
        if tables.iter().any(|table| table.enabled) {
            sqlx::query("SELECT 1 FROM cloudsync_changes LIMIT 0")
                .fetch_optional(&mut *connection)
                .await?;
        }
        if extension_path.is_some() {
            validate_extension(connection).await?;
        }

        Ok(())
    }
}

fn extension_load_lock_for(database_path: PathBuf) -> Arc<ExtensionLoadLock> {
    let locks = EXTENSION_LOAD_LOCKS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut locks = locks.lock().unwrap();
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(&database_path).and_then(Weak::upgrade) {
        return lock;
    }

    let lock = Arc::new(ExtensionLoadLock::new(()));
    locks.insert(database_path, Arc::downgrade(&lock));
    lock
}

async fn load_and_validate_extension(
    connection: &mut SqliteConnection,
    extension_path: &Path,
) -> Result<(), Error> {
    // Loading the extension is per-connection and does not need a reserved
    // lock. BEGIN IMMEDIATE here fights in-flight writers and makes sqlx log
    // after_connect SQLITE_BUSY while the pool grows.
    sqlx::query("PRAGMA busy_timeout = 50")
        .execute(&mut *connection)
        .await?;

    let deadline = tokio::time::Instant::now() + EXTENSION_LOAD_BUSY_TIMEOUT;
    let mut attempt = 0u32;
    let result = loop {
        match load_extension_and_validate(connection, extension_path).await {
            Ok(()) => break Ok(()),
            Err(error) if is_busy_error(&error) => {
                let now = tokio::time::Instant::now();
                if now >= deadline {
                    break Err(error);
                }
                let delay = Duration::from_millis(10_u64 << attempt.min(5));
                tokio::time::sleep(delay.min(deadline - now)).await;
                attempt = attempt.saturating_add(1);
            }
            Err(error) => break Err(error),
        }
    };
    restore_busy_timeout(connection).await?;
    result
}

async fn load_extension_and_validate(
    connection: &mut SqliteConnection,
    extension_path: &Path,
) -> Result<(), Error> {
    load_extension(connection, extension_path).await?;
    validate_extension(connection).await
}

fn is_sqlite_busy(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|error| {
        matches!(error.code().as_deref(), Some("5" | "6")) || {
            let message = error.message().to_ascii_lowercase();
            message.contains("database is locked") || message.contains("database table is locked")
        }
    })
}

fn is_busy_error(error: &Error) -> bool {
    match error {
        Error::Sqlx(error) => is_sqlite_busy(error),
        error => {
            let message = error.to_string().to_ascii_lowercase();
            message.contains("database is locked")
                || message.contains("database table is locked")
                || message.contains("sqlite error 5")
                || message.contains("sqlite error 6")
        }
    }
}

async fn restore_busy_timeout(connection: &mut SqliteConnection) -> Result<(), Error> {
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(connection)
        .await?;
    Ok(())
}

async fn validate_extension(connection: &mut SqliteConnection) -> Result<(), Error> {
    let loaded_version = version(&mut *connection).await?;
    if loaded_version != crate::CLOUDSYNC_VERSION {
        return Err(Error::ExtensionInitialization(format!(
            "expected sqlite-sync {}, loaded {loaded_version}",
            crate::CLOUDSYNC_VERSION
        )));
    }

    let internal_table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM sqlite_master
         WHERE type = 'table'
           AND name IN (
             'cloudsync_site_id',
             'cloudsync_settings',
             'cloudsync_table_settings',
             'cloudsync_schema_versions'
           )",
    )
    .fetch_one(&mut *connection)
    .await?;
    if internal_table_count == 0 {
        return Ok(());
    }
    if internal_table_count != 4 {
        return Err(Error::ExtensionInitialization(format!(
            "sqlite-sync internal schema is incomplete ({internal_table_count}/4 tables)"
        )));
    }

    let stored_version: Option<String> = sqlx::query_scalar(
        "SELECT value FROM cloudsync_settings WHERE key = 'version' COLLATE NOCASE",
    )
    .fetch_optional(&mut *connection)
    .await?;
    if stored_version.as_deref() != Some(crate::CLOUDSYNC_VERSION) {
        return Err(Error::ExtensionInitialization(
            "sqlite-sync settings version is missing or stale".to_string(),
        ));
    }

    let stored_site_id: Vec<u8> =
        sqlx::query_scalar("SELECT site_id FROM cloudsync_site_id WHERE rowid = 0")
            .fetch_one(&mut *connection)
            .await?;
    let context_site_id = siteid(&mut *connection).await?;
    if context_site_id != stored_site_id {
        return Err(Error::ExtensionInitialization(
            "sqlite-sync connection context does not match the persisted site ID".to_string(),
        ));
    }

    Ok(())
}

#[allow(unsafe_code)]
async fn load_extension(
    connection: &mut SqliteConnection,
    extension_path: &Path,
) -> Result<(), Error> {
    use std::ffi::{CStr, CString, c_int};
    use std::ptr;

    use libsqlite3_sys::{
        SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION, SQLITE_OK, sqlite3_db_config, sqlite3_free,
        sqlite3_load_extension,
    };

    let extension_path =
        CString::new(extension_path.to_string_lossy().as_bytes()).map_err(|_| {
            Error::ExtensionInitialization(
                "sqlite-sync extension path contains a NUL byte".to_string(),
            )
        })?;
    let mut handle = connection.lock_handle().await?;
    let database = handle.as_raw_handle().as_ptr();

    // Loading is enabled only around the trusted bundled extension.
    let enable_result = unsafe {
        sqlite3_db_config(
            database,
            SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION,
            1,
            ptr::null_mut::<c_int>(),
        )
    };
    if enable_result != SQLITE_OK {
        return Err(Error::ExtensionInitialization(format!(
            "failed to enable sqlite-sync loading (sqlite error {enable_result})"
        )));
    }

    let mut error_message = ptr::null_mut();
    let load_result = unsafe {
        sqlite3_load_extension(
            database,
            extension_path.as_ptr(),
            ptr::null(),
            &mut error_message,
        )
    };
    let message = (!error_message.is_null()).then(|| {
        let message = unsafe { CStr::from_ptr(error_message) }
            .to_string_lossy()
            .into_owned();
        unsafe {
            sqlite3_free(error_message.cast());
        }
        message
    });
    let disable_result = unsafe {
        sqlite3_db_config(
            database,
            SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION,
            0,
            ptr::null_mut::<c_int>(),
        )
    };

    if load_result != SQLITE_OK {
        return Err(Error::ExtensionInitialization(format!(
            "failed to load sqlite-sync (sqlite error {load_result}): {}",
            message.unwrap_or_else(|| "no error message".to_string())
        )));
    }
    if disable_result != SQLITE_OK {
        return Err(Error::ExtensionInitialization(format!(
            "failed to disable sqlite-sync loading (sqlite error {disable_result})"
        )));
    }
    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-version
pub async fn version<'e, E>(executor: E) -> Result<String, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    Ok(sqlx::query_scalar("SELECT cloudsync_version()")
        .fetch_one(executor)
        .await?)
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-init
pub async fn init<'e, E>(
    executor: E,
    table_name: &str,
    crdt_algo: Option<&str>,
    init_flags: Option<i64>,
) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    match (crdt_algo, init_flags) {
        (None, None) => {
            sqlx::query("SELECT cloudsync_init(?)")
                .bind(table_name)
                .fetch_optional(executor)
                .await?;
        }
        (Some(crdt_algo), None) => {
            sqlx::query("SELECT cloudsync_init(?, ?)")
                .bind(table_name)
                .bind(crdt_algo)
                .fetch_optional(executor)
                .await?;
        }
        (None, Some(init_flags)) => {
            sqlx::query("SELECT cloudsync_init(?, NULL, ?)")
                .bind(table_name)
                .bind(init_flags)
                .fetch_optional(executor)
                .await?;
        }
        (Some(crdt_algo), Some(init_flags)) => {
            sqlx::query("SELECT cloudsync_init(?, ?, ?)")
                .bind(table_name)
                .bind(crdt_algo)
                .bind(init_flags)
                .fetch_optional(executor)
                .await?;
        }
    }

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-begin-alter
pub async fn begin_alter<'e, E>(executor: E, table_name: &str) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT cloudsync_begin_alter(?)")
        .bind(table_name)
        .fetch_optional(executor)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-enable
pub async fn enable<'e, E>(executor: E, table_name: &str) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT cloudsync_enable(?)")
        .bind(table_name)
        .fetch_optional(executor)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-disable
pub async fn disable<'e, E>(executor: E, table_name: &str) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT cloudsync_disable(?)")
        .bind(table_name)
        .fetch_optional(executor)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-is-enabled
pub async fn is_enabled<'e, E>(executor: E, table_name: &str) -> Result<bool, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    Ok(sqlx::query_scalar("SELECT cloudsync_is_enabled(?)")
        .bind(table_name)
        .fetch_one(executor)
        .await?)
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-set-filter
pub async fn set_filter<'e, E>(
    executor: E,
    table_name: &str,
    filter_expression: &str,
) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT cloudsync_set_filter(?, ?)")
        .bind(table_name)
        .bind(filter_expression)
        .fetch_optional(executor)
        .await?;
    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-commit-alter
pub async fn commit_alter<'e, E>(executor: E, table_name: &str) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT cloudsync_commit_alter(?)")
        .bind(table_name)
        .fetch_optional(executor)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-cleanup
pub async fn cleanup<'e, E>(executor: E, table_name: &str) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT cloudsync_cleanup(?)")
        .bind(table_name)
        .fetch_optional(executor)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-siteid
pub async fn siteid<'e, E>(executor: E) -> Result<Vec<u8>, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    Ok(sqlx::query_scalar("SELECT cloudsync_siteid()")
        .fetch_one(executor)
        .await?)
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-db-version
pub async fn db_version<'e, E>(executor: E) -> Result<i64, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    Ok(sqlx::query_scalar("SELECT cloudsync_db_version()")
        .fetch_one(executor)
        .await?)
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-uuid
pub async fn uuid<'e, E>(executor: E) -> Result<String, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    Ok(sqlx::query_scalar("SELECT cloudsync_uuid()")
        .fetch_one(executor)
        .await?)
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-terminate
pub async fn terminate<'e, E>(executor: E) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT cloudsync_terminate()")
        .fetch_optional(executor)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_busy_messages_are_retried() {
        assert!(is_busy_error(&Error::ExtensionInitialization(
            "failed to load sqlite-sync (sqlite error 5): database is locked".to_string(),
        )));
        assert!(!is_busy_error(&Error::ExtensionInitialization(
            "expected sqlite-sync 1.1.2, loaded 1.0.0".to_string(),
        )));
    }
}

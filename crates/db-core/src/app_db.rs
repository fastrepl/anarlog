//! How the desktop apps open `app.db`: with the bundled sqlite-sync extension, whose triggers
//! and migrations synced tables need, and in local-only mode only when the extension can't
//! load and the database never ran sqlite-sync.

use crate::{Db, DbOpenError, DbOpenOptions, DbStorage};

/// The shipping desktop app's pool size.
pub const APP_DB_MAX_CONNECTIONS: u32 = 4;

pub fn app_db_open_options(
    storage: DbStorage<'_>,
    cloudsync_enabled: bool,
    max_connections: u32,
) -> DbOpenOptions<'_> {
    DbOpenOptions {
        storage,
        cloudsync_enabled,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(max_connections),
    }
}

/// Opens the app database without preparing its schema.
pub async fn open_app_db(storage: DbStorage<'_>, max_connections: u32) -> Result<Db, DbOpenError> {
    match Db::open(app_db_open_options(storage, true, max_connections)).await {
        Ok(db) => Ok(db),
        Err(cloudsync_error) => {
            let probe_error = match probe_cloudsync_extension().await {
                Ok(()) => return Err(cloudsync_error),
                Err(error) => error,
            };
            open_app_db_without_cloudsync(storage, max_connections, cloudsync_error, probe_error)
                .await
        }
    }
}

async fn probe_cloudsync_extension() -> Result<(), DbOpenError> {
    let db = Db::open(app_db_open_options(DbStorage::Memory, true, 1)).await?;
    db.pool().close().await;
    Ok(())
}

/// The local-only fallback once the extension failed to load, refused for a database that
/// sqlite-sync initialized.
pub async fn open_app_db_without_cloudsync(
    storage: DbStorage<'_>,
    max_connections: u32,
    cloudsync_error: DbOpenError,
    probe_error: DbOpenError,
) -> Result<Db, DbOpenError> {
    let db = Db::open(app_db_open_options(storage, false, max_connections)).await?;
    if database_uses_cloudsync_schema(&db).await? {
        db.pool().close().await;
        tracing::error!(
            %cloudsync_error,
            %probe_error,
            "cloudsync extension is unavailable for an initialized local replica"
        );
        return Err(cloudsync_error);
    }

    tracing::warn!(
        %cloudsync_error,
        %probe_error,
        "cloudsync extension is unavailable; opened the app database in local-only mode"
    );
    Ok(db)
}

// App triggers may read the cloudsync_workspace_binding setting; only the
// extension's own triggers, which reference cloudsync_ tables, mean the
// database was initialized for sqlite-sync.
pub async fn database_uses_cloudsync_schema(db: &Db) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM sqlite_master
            WHERE (type = 'table' AND name = 'cloudsync_table_settings')
               OR (
                 type = 'trigger'
                 AND instr(
                   replace(lower(COALESCE(sql, '')), 'cloudsync_workspace_binding', ''),
                   'cloudsync_'
                 ) > 0
               )
        )",
    )
    .fetch_one(db.pool())
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unavailable_extension_error() -> DbOpenError {
        DbOpenError::Io(std::io::Error::other("cloudsync extension unavailable"))
    }

    fn failed_extension_probe_error() -> DbOpenError {
        DbOpenError::Io(std::io::Error::other("cloudsync extension probe failed"))
    }

    async fn initialized_replica(db_path: &std::path::Path) {
        let db = Db::open(app_db_open_options(DbStorage::Local(db_path), true, 1))
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL DEFAULT '')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.cloudsync_init("items", None, None).await.unwrap();
        db.pool().close().await;
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
    async fn extension_open_without_initialized_tables_allows_plain_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = Db::open(app_db_open_options(DbStorage::Local(&db_path), true, 1))
            .await
            .unwrap();
        db.pool().close().await;
        drop(db);

        let db = open_app_db_without_cloudsync(
            DbStorage::Local(&db_path),
            1,
            unavailable_extension_error(),
            failed_extension_probe_error(),
        )
        .await
        .unwrap();

        assert!(!db.cloudsync_enabled());
        assert!(!database_uses_cloudsync_schema(&db).await.unwrap());
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
    async fn an_initialized_replica_opens_only_with_the_extension() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        initialized_replica(&db_path).await;

        let error = open_app_db_without_cloudsync(
            DbStorage::Local(&db_path),
            1,
            unavailable_extension_error(),
            failed_extension_probe_error(),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, DbOpenError::Io(_)));

        let db = open_app_db(DbStorage::Local(&db_path), 1).await.unwrap();
        assert!(db.cloudsync_enabled());
        assert!(database_uses_cloudsync_schema(&db).await.unwrap());
    }
}

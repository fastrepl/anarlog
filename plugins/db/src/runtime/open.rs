use std::path::Path;

use anlg_db_core::{Db, DbOpenError, DbOpenOptions, DbStorage};

use crate::Result;

pub async fn open_app_db(db_path: Option<&Path>) -> Result<Db> {
    open_app_db_with(db_path, true).await
}

pub async fn open_app_db_unmigrated(db_path: Option<&Path>) -> Result<Db> {
    open_app_db_with(db_path, false).await
}

async fn open_app_db_with(db_path: Option<&Path>, prepare_schema: bool) -> Result<Db> {
    let storage = match db_path {
        Some(path) => DbStorage::Local(path),
        None => DbStorage::Memory,
    };

    match Db::open(app_db_open_options(storage, true)).await {
        Ok(db) => {
            if prepare_schema {
                anlg_db_app::prepare_schema(&db).await?;
            }
            Ok(db)
        }
        Err(cloudsync_error) => {
            let probe_error = match probe_cloudsync_extension().await {
                Ok(()) => return Err(cloudsync_error.into()),
                Err(error) => error,
            };
            open_app_db_without_cloudsync(storage, cloudsync_error, probe_error, prepare_schema)
                .await
        }
    }
}

pub(super) fn app_db_open_options(
    storage: DbStorage<'_>,
    cloudsync_enabled: bool,
) -> DbOpenOptions<'_> {
    DbOpenOptions {
        storage,
        cloudsync_enabled,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    }
}

async fn probe_cloudsync_extension() -> std::result::Result<(), DbOpenError> {
    let db = Db::open(app_db_open_options(DbStorage::Memory, true)).await?;
    db.pool().close().await;
    Ok(())
}

pub(super) async fn open_app_db_without_cloudsync(
    storage: DbStorage<'_>,
    cloudsync_error: DbOpenError,
    probe_error: DbOpenError,
    prepare_schema: bool,
) -> Result<Db> {
    let db = Db::open(app_db_open_options(storage, false)).await?;
    if database_uses_cloudsync_schema(&db).await? {
        db.pool().close().await;
        tracing::error!(
            %cloudsync_error,
            %probe_error,
            "cloudsync extension is unavailable for an initialized local replica"
        );
        return Err(cloudsync_error.into());
    }

    if prepare_schema && let Err(error) = anlg_db_app::prepare_schema(&db).await {
        db.pool().close().await;
        return Err(error.into());
    }

    tracing::warn!(
        %cloudsync_error,
        %probe_error,
        "cloudsync extension is unavailable; opened the app database in local-only mode"
    );
    Ok(db)
}

pub(super) async fn database_uses_cloudsync_schema(
    db: &Db,
) -> std::result::Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM sqlite_master
            WHERE (type = 'table' AND name = 'cloudsync_table_settings')
               OR (type = 'trigger' AND instr(lower(COALESCE(sql, '')), 'cloudsync_') > 0)
        )",
    )
    .fetch_one(db.pool())
    .await
}

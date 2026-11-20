use std::path::Path;

#[cfg(test)]
use anlg_db_core::DbOpenError;
use anlg_db_core::{APP_DB_MAX_CONNECTIONS, Db, DbStorage};

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
    let db = anlg_db_core::open_app_db(storage, APP_DB_MAX_CONNECTIONS).await?;
    prepared(db, prepare_schema).await
}

#[cfg(test)]
pub(super) fn app_db_open_options(
    storage: DbStorage<'_>,
    cloudsync_enabled: bool,
) -> anlg_db_core::DbOpenOptions<'_> {
    anlg_db_core::app_db_open_options(storage, cloudsync_enabled, APP_DB_MAX_CONNECTIONS)
}

#[cfg(test)]
pub(super) async fn open_app_db_without_cloudsync(
    storage: DbStorage<'_>,
    cloudsync_error: DbOpenError,
    probe_error: DbOpenError,
    prepare_schema: bool,
) -> Result<Db> {
    let db = anlg_db_core::open_app_db_without_cloudsync(
        storage,
        APP_DB_MAX_CONNECTIONS,
        cloudsync_error,
        probe_error,
    )
    .await?;
    prepared(db, prepare_schema).await
}

async fn prepared(db: Db, prepare_schema: bool) -> Result<Db> {
    if prepare_schema && let Err(error) = anlg_db_app::prepare_schema(&db).await {
        db.pool().close().await;
        return Err(error.into());
    }
    Ok(db)
}

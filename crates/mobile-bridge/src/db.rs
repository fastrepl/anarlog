use std::path::PathBuf;

use hypr_db_core::{Db, DbOpenOptions, DbStorage};

use crate::error::OpenAppDbError;

pub(crate) async fn open_app_db(
    db_path: &PathBuf,
    cloudsync_enabled: bool,
) -> Result<Db, OpenAppDbError> {
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Local(db_path),
        cloudsync_enabled,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await?;

    hypr_db_migrate::migrate(&db, hypr_db_app::schema()).await?;

    Ok(db)
}

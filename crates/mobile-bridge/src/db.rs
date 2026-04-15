use std::path::PathBuf;

use hypr_db_core2::{CloudsyncOpenMode, Db3, DbOpenOptions, DbStorage};
use hypr_db_migrate::AppDbOpenError;

pub(crate) async fn open_app_db(
    db_path: &PathBuf,
    cloudsync_open_mode: CloudsyncOpenMode,
) -> Result<Db3, AppDbOpenError> {
    hypr_db_migrate::open_db(
        hypr_db_migrate::AppDbOpenOptions {
            db: DbOpenOptions {
                storage: DbStorage::Local(db_path),
                cloudsync_open_mode,
                journal_mode_wal: true,
                foreign_keys: true,
                max_connections: Some(4),
            },
            migration_failure_policy: hypr_db_migrate::MigrationFailurePolicy::Fail,
        },
        hypr_db_app::schema(),
    )
    .await
}

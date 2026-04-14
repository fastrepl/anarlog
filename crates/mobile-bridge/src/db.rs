use std::path::PathBuf;

use hypr_db_core2::{CloudsyncOpenMode, Db3, DbOpenOptions, DbStorage, MigrationFailurePolicy};

pub(crate) async fn open_app_db(
    db_path: &PathBuf,
    cloudsync_open_mode: CloudsyncOpenMode,
) -> Result<Db3, hypr_db_core2::DbOpenError> {
    Db3::open_with_migrate(
        DbOpenOptions {
            storage: DbStorage::Local(db_path),
            cloudsync_open_mode,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(4),
            migration_failure_policy: MigrationFailurePolicy::Fail,
        },
        |pool| Box::pin(hypr_db_app::migrate(pool)),
    )
    .await
}

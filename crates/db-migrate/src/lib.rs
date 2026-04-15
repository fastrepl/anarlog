#![forbid(unsafe_code)]

mod error;
mod migrate;
mod schema;

pub use error::AppDbOpenError;
pub use schema::{
    AppDbOpenOptions, DbSchema, MigrationFailurePolicy, MigrationScope, MigrationStep,
};

use hypr_db_core2::{CloudsyncOpenMode, Db3, DbStorage};

pub async fn open_db(
    options: AppDbOpenOptions<'_>,
    schema: DbSchema,
) -> Result<Db3, AppDbOpenError> {
    if options.db.cloudsync_open_mode == CloudsyncOpenMode::Enabled
        && matches!(
            options.migration_failure_policy,
            MigrationFailurePolicy::Recreate
        )
    {
        return Err(AppDbOpenError::RecreateNotAllowedWithCloudsync);
    }

    match try_open_db(options, schema).await {
        Ok(db) => Ok(db),
        Err(_error)
            if matches!(
                options.migration_failure_policy,
                MigrationFailurePolicy::Recreate
            ) && options.db.cloudsync_open_mode == CloudsyncOpenMode::Disabled
                && matches!(options.db.storage, DbStorage::Local(_)) =>
        {
            hypr_db_core2::recreate_storage(&options.db)?;
            try_open_db(options, schema).await
        }
        Err(error) => Err(error),
    }
}

async fn try_open_db(
    options: AppDbOpenOptions<'_>,
    schema: DbSchema,
) -> Result<Db3, AppDbOpenError> {
    let db = Db3::open(options.db).await?;

    if let Err(error) = migrate::run_migrations(&db, schema).await {
        db.pool().clone().close().await;
        return Err(error);
    }

    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core2::DbOpenOptions;

    fn empty_schema() -> DbSchema {
        DbSchema {
            steps: &[],
            validate_cloudsync_table: |_table| false,
        }
    }

    fn test_options<'a>(
        storage: DbStorage<'a>,
        cloudsync_open_mode: CloudsyncOpenMode,
    ) -> AppDbOpenOptions<'a> {
        AppDbOpenOptions {
            db: DbOpenOptions {
                storage,
                cloudsync_open_mode,
                journal_mode_wal: true,
                foreign_keys: true,
                max_connections: Some(4),
            },
            migration_failure_policy: MigrationFailurePolicy::Fail,
        }
    }

    #[tokio::test]
    async fn recreate_is_rejected_for_cloudsync_open_mode() {
        let error = open_db(
            AppDbOpenOptions {
                migration_failure_policy: MigrationFailurePolicy::Recreate,
                ..test_options(DbStorage::Memory, CloudsyncOpenMode::Enabled)
            },
            empty_schema(),
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            AppDbOpenError::RecreateNotAllowedWithCloudsync
        ));
    }

    #[tokio::test]
    async fn open_db_bootstraps_app_migration_history() {
        let db = open_db(
            test_options(DbStorage::Memory, CloudsyncOpenMode::Disabled),
            empty_schema(),
        )
        .await
        .unwrap();

        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '_sqlx%' ORDER BY name",
        )
        .fetch_all(db.pool().as_ref())
        .await
        .unwrap();

        assert!(tables.contains(&"app_migrations".to_string()));
    }
}

#![forbid(unsafe_code)]

mod error;
mod migrate;
mod schema;

pub use error::MigrateError;
pub use schema::{DbSchema, MigrationScope, MigrationStep};

use hypr_db_core2::Db3;

pub async fn migrate(db: &Db3, schema: DbSchema) -> Result<(), MigrateError> {
    migrate::run_migrations(db, schema).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core2::{DbOpenOptions, DbStorage};

    fn empty_schema() -> DbSchema {
        DbSchema {
            steps: &[],
            validate_cloudsync_table: |_table| false,
        }
    }

    #[tokio::test]
    async fn migrate_bootstraps_migration_history() {
        let db = Db3::open(DbOpenOptions {
            storage: DbStorage::Memory,
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();

        migrate(&db, empty_schema()).await.unwrap();

        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(db.pool().as_ref())
        .await
        .unwrap();

        assert!(tables.contains(&"_sqlx_migrations".to_string()));
    }
}

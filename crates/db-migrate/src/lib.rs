#![forbid(unsafe_code)]

mod error;
mod migrate;
mod schema;

pub use error::MigrateError;
pub use schema::{DbSchema, MigrationScope, MigrationStep};

use anlg_db_core::Db;

pub async fn migrate(db: &Db, schema: DbSchema) -> Result<(), MigrateError> {
    migrate::run_migrations(db, schema).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_db_core::{DbOpenOptions, DbStorage};

    fn empty_schema() -> DbSchema {
        DbSchema {
            steps: &[],
            validate_cloudsync_table: |_table| false,
        }
    }

    fn schema_of(steps: &'static [MigrationStep]) -> DbSchema {
        DbSchema {
            steps,
            validate_cloudsync_table: |_table| false,
        }
    }

    async fn open_memory_db() -> Db {
        Db::open(DbOpenOptions {
            storage: DbStorage::Memory,
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap()
    }

    const STEP_ONE: MigrationStep = MigrationStep {
        id: "10_one",
        scope: MigrationScope::Plain,
        sql: "CREATE TABLE t_one (id INTEGER PRIMARY KEY);",
    };
    const STEP_TWO_ADDITIVE: MigrationStep = MigrationStep {
        id: "20_two",
        scope: MigrationScope::Plain,
        sql: "CREATE TABLE t_two (id INTEGER PRIMARY KEY);",
    };
    const STEP_TWO_BREAKING: MigrationStep = MigrationStep {
        id: "20_two",
        scope: MigrationScope::Plain,
        sql: "-- reworks t_one in a way older builds cannot read\n-- breaking\nCREATE TABLE t_two (id INTEGER PRIMARY KEY);",
    };
    const STEP_THREE_ADDITIVE: MigrationStep = MigrationStep {
        id: "30_three",
        scope: MigrationScope::Plain,
        sql: "CREATE TABLE t_three (id INTEGER PRIMARY KEY);",
    };

    #[tokio::test]
    async fn older_build_tolerates_newer_additive_migrations() {
        let db = open_memory_db().await;
        migrate(&db, schema_of(&[STEP_ONE, STEP_TWO_ADDITIVE]))
            .await
            .unwrap();

        migrate(&db, schema_of(&[STEP_ONE])).await.unwrap();

        let recorded: Vec<i64> =
            sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(recorded, vec![10, 20]);
    }

    #[tokio::test]
    async fn newer_database_rejects_missing_local_migration() {
        let db = open_memory_db().await;
        migrate(&db, schema_of(&[STEP_ONE, STEP_THREE_ADDITIVE]))
            .await
            .unwrap();

        let error = migrate(&db, schema_of(&[STEP_ONE, STEP_TWO_ADDITIVE]))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            MigrateError::DatabaseAhead {
                missing_version: 20,
                max_applied_version: 30,
            }
        ));
        assert!(
            error
                .to_string()
                .contains("created by a newer version of Anarlog")
        );

        let recorded: Vec<i64> =
            sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(recorded, vec![10, 30]);

        let table_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 't_two'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(table_count, 0);
    }

    #[tokio::test]
    async fn breaking_migration_blocks_older_builds() {
        let db = open_memory_db().await;
        migrate(&db, schema_of(&[STEP_ONE, STEP_TWO_BREAKING]))
            .await
            .unwrap();

        let error = migrate(&db, schema_of(&[STEP_ONE])).await.unwrap_err();

        assert!(matches!(
            error,
            MigrateError::SchemaFromNewerApp {
                min_supported_version: 20,
                max_known_version: 10,
            }
        ));
        assert!(
            error
                .to_string()
                .contains("created by a newer version of Anarlog")
        );
    }

    #[tokio::test]
    async fn breaking_floor_allows_builds_that_include_it() {
        let db = open_memory_db().await;
        migrate(
            &db,
            schema_of(&[STEP_ONE, STEP_TWO_BREAKING, STEP_THREE_ADDITIVE]),
        )
        .await
        .unwrap();

        migrate(&db, schema_of(&[STEP_ONE, STEP_TWO_BREAKING]))
            .await
            .unwrap();

        let floor: i64 = sqlx::query_scalar(
            "SELECT min_supported_version FROM _anlg_schema_compat WHERE id = 0",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(floor, 20);
    }

    #[tokio::test]
    async fn unknown_migration_below_newest_known_still_fails() {
        let db = open_memory_db().await;
        migrate(&db, schema_of(&[STEP_ONE, STEP_TWO_ADDITIVE]))
            .await
            .unwrap();

        let error = migrate(&db, schema_of(&[STEP_TWO_ADDITIVE]))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            MigrateError::SqlxMigrate(sqlx::migrate::MigrateError::VersionMissing(10))
        ));
    }

    #[tokio::test]
    async fn migrate_bootstraps_migration_history() {
        let db = Db::open(DbOpenOptions {
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
        .fetch_all(db.pool())
        .await
        .unwrap();

        assert!(tables.contains(&"_sqlx_migrations".to_string()));
    }
}

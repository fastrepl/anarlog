use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use sqlx::{Executor, Sqlite, SqliteConnection};

use crate::error::Error;

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
}

impl CloudsyncConnectionInitializer {
    pub fn replace_tables(&self, tables: Vec<CloudsyncTableSpec>) {
        *self.tables.write().unwrap() = tables;
    }

    pub fn clear(&self) {
        self.tables.write().unwrap().clear();
    }

    pub async fn initialize(&self, connection: &mut SqliteConnection) -> Result<(), Error> {
        let tables = self.tables.read().unwrap().clone();
        for table in tables.iter().filter(|table| table.enabled) {
            init(
                &mut *connection,
                &table.table_name,
                table.crdt_algo.as_deref(),
                table.init_flags,
            )
            .await?;
        }

        Ok(())
    }
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

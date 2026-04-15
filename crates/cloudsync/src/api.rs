use sqlx::{Executor, Sqlite};

use crate::error::Error;

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
    force: Option<bool>,
) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite> + Copy,
{
    match (crdt_algo, force) {
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
        (None, Some(force)) => {
            sqlx::query("SELECT cloudsync_init(?, NULL, ?)")
                .bind(table_name)
                .bind(force)
                .fetch_optional(executor)
                .await?;
        }
        (Some(crdt_algo), Some(force)) => {
            sqlx::query("SELECT cloudsync_init(?, ?, ?)")
                .bind(table_name)
                .bind(crdt_algo)
                .bind(force)
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

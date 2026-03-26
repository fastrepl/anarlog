use sqlx::SqlitePool;

use crate::error::Error;

pub async fn version(pool: &SqlitePool) -> Result<String, Error> {
    Ok(sqlx::query_scalar("SELECT cloudsync_version()")
        .fetch_one(pool)
        .await?)
}

pub async fn init(
    pool: &SqlitePool,
    table_name: &str,
    crdt_algo: Option<&str>,
    force: Option<bool>,
) -> Result<(), Error> {
    match (crdt_algo, force) {
        (None, None) => {
            sqlx::query("SELECT cloudsync_init(?)")
                .bind(table_name)
                .fetch_optional(pool)
                .await?;
        }
        (Some(crdt_algo), None) => {
            sqlx::query("SELECT cloudsync_init(?, ?)")
                .bind(table_name)
                .bind(crdt_algo)
                .fetch_optional(pool)
                .await?;
        }
        (None, Some(force)) => {
            sqlx::query("SELECT cloudsync_init(?, NULL, ?)")
                .bind(table_name)
                .bind(force)
                .fetch_optional(pool)
                .await?;
        }
        (Some(crdt_algo), Some(force)) => {
            sqlx::query("SELECT cloudsync_init(?, ?, ?)")
                .bind(table_name)
                .bind(crdt_algo)
                .bind(force)
                .fetch_optional(pool)
                .await?;
        }
    }

    Ok(())
}

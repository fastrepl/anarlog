use sqlx::SqlitePool;

use crate::error::Error;

pub async fn network_init(pool: &SqlitePool, connection_string: &str) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_init(?)")
        .bind(connection_string)
        .fetch_optional(pool)
        .await?;

    Ok(())
}

pub async fn network_set_apikey(pool: &SqlitePool, api_key: &str) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_set_apikey(?)")
        .bind(api_key)
        .fetch_optional(pool)
        .await?;

    Ok(())
}

pub async fn network_set_token(pool: &SqlitePool, token: &str) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_set_token(?)")
        .bind(token)
        .fetch_optional(pool)
        .await?;

    Ok(())
}

pub async fn network_sync(
    pool: &SqlitePool,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<(), Error> {
    match (wait_ms, max_retries) {
        (None, None) => {
            sqlx::query("SELECT cloudsync_network_sync()")
                .fetch_optional(pool)
                .await?;
        }
        (Some(wait_ms), None) => {
            sqlx::query("SELECT cloudsync_network_sync(?)")
                .bind(wait_ms)
                .fetch_optional(pool)
                .await?;
        }
        (None, Some(max_retries)) => {
            sqlx::query("SELECT cloudsync_network_sync(NULL, ?)")
                .bind(max_retries)
                .fetch_optional(pool)
                .await?;
        }
        (Some(wait_ms), Some(max_retries)) => {
            sqlx::query("SELECT cloudsync_network_sync(?, ?)")
                .bind(wait_ms)
                .bind(max_retries)
                .fetch_optional(pool)
                .await?;
        }
    }

    Ok(())
}

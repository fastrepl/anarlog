use sqlx::SqlitePool;

use crate::error::Error;

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-init
pub async fn network_init(pool: &SqlitePool, connection_string: &str) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_init(?)")
        .bind(connection_string)
        .fetch_optional(pool)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-set-apikey
pub async fn network_set_apikey(pool: &SqlitePool, api_key: &str) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_set_apikey(?)")
        .bind(api_key)
        .fetch_optional(pool)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-set-token
pub async fn network_set_token(pool: &SqlitePool, token: &str) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_set_token(?)")
        .bind(token)
        .fetch_optional(pool)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-cleanup
pub async fn network_cleanup(pool: &SqlitePool) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_cleanup()")
        .fetch_optional(pool)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-has-unsent-changes
pub async fn network_has_unsent_changes(pool: &SqlitePool) -> Result<bool, Error> {
    Ok(
        sqlx::query_scalar("SELECT cloudsync_network_has_unsent_changes()")
            .fetch_one(pool)
            .await?,
    )
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-send-changes
pub async fn network_send_changes(
    pool: &SqlitePool,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<i64, Error> {
    Ok(match (wait_ms, max_retries) {
        (None, None) => {
            sqlx::query_scalar("SELECT cloudsync_network_send_changes()")
                .fetch_one(pool)
                .await?
        }
        (Some(wait_ms), None) => {
            sqlx::query_scalar("SELECT cloudsync_network_send_changes(?)")
                .bind(wait_ms)
                .fetch_one(pool)
                .await?
        }
        (None, Some(max_retries)) => {
            sqlx::query_scalar("SELECT cloudsync_network_send_changes(NULL, ?)")
                .bind(max_retries)
                .fetch_one(pool)
                .await?
        }
        (Some(wait_ms), Some(max_retries)) => {
            sqlx::query_scalar("SELECT cloudsync_network_send_changes(?, ?)")
                .bind(wait_ms)
                .bind(max_retries)
                .fetch_one(pool)
                .await?
        }
    })
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-check-changes
pub async fn network_check_changes(
    pool: &SqlitePool,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<i64, Error> {
    Ok(match (wait_ms, max_retries) {
        (None, None) => {
            sqlx::query_scalar("SELECT cloudsync_network_check_changes()")
                .fetch_one(pool)
                .await?
        }
        (Some(wait_ms), None) => {
            sqlx::query_scalar("SELECT cloudsync_network_check_changes(?)")
                .bind(wait_ms)
                .fetch_one(pool)
                .await?
        }
        (None, Some(max_retries)) => {
            sqlx::query_scalar("SELECT cloudsync_network_check_changes(NULL, ?)")
                .bind(max_retries)
                .fetch_one(pool)
                .await?
        }
        (Some(wait_ms), Some(max_retries)) => {
            sqlx::query_scalar("SELECT cloudsync_network_check_changes(?, ?)")
                .bind(wait_ms)
                .bind(max_retries)
                .fetch_one(pool)
                .await?
        }
    })
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-reset-sync-version
pub async fn network_reset_sync_version(pool: &SqlitePool) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_reset_sync_version()")
        .fetch_optional(pool)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-logout
pub async fn network_logout(pool: &SqlitePool) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_logout()")
        .fetch_optional(pool)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-sync
pub async fn network_sync(
    pool: &SqlitePool,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<i64, Error> {
    Ok(match (wait_ms, max_retries) {
        (None, None) => {
            sqlx::query_scalar("SELECT cloudsync_network_sync()")
                .fetch_one(pool)
                .await?
        }
        (Some(wait_ms), None) => {
            sqlx::query_scalar("SELECT cloudsync_network_sync(?)")
                .bind(wait_ms)
                .fetch_one(pool)
                .await?
        }
        (None, Some(max_retries)) => {
            sqlx::query_scalar("SELECT cloudsync_network_sync(NULL, ?)")
                .bind(max_retries)
                .fetch_one(pool)
                .await?
        }
        (Some(wait_ms), Some(max_retries)) => {
            sqlx::query_scalar("SELECT cloudsync_network_sync(?, ?)")
                .bind(wait_ms)
                .bind(max_retries)
                .fetch_one(pool)
                .await?
        }
    })
}

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::Error;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NetworkResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send: Option<NetworkSendResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receive: Option<NetworkReceiveResult>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSendResult {
    pub status: String,
    pub local_version: i64,
    pub server_version: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failure: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkReceiveResult {
    pub rows: i64,
    pub tables: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failure: Option<serde_json::Value>,
}

async fn query_with_optional_params(
    pool: &SqlitePool,
    fn_name: &str,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<NetworkResult, Error> {
    let response: String = match (wait_ms, max_retries) {
        (None, None) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {fn_name}()")))
                .fetch_one(pool)
                .await?
        }
        (Some(wait_ms), None) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {fn_name}(?)")))
                .bind(wait_ms)
                .fetch_one(pool)
                .await?
        }
        (None, Some(max_retries)) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {fn_name}(NULL, ?)")))
                .bind(max_retries)
                .fetch_one(pool)
                .await?
        }
        (Some(wait_ms), Some(max_retries)) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {fn_name}(?, ?)")))
                .bind(wait_ms)
                .bind(max_retries)
                .fetch_one(pool)
                .await?
        }
    };

    Ok(serde_json::from_str(&response)?)
}

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
) -> Result<NetworkResult, Error> {
    query_with_optional_params(pool, "cloudsync_network_send_changes", wait_ms, max_retries).await
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-check-changes
pub async fn network_check_changes(
    pool: &SqlitePool,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<NetworkResult, Error> {
    query_with_optional_params(
        pool,
        "cloudsync_network_check_changes",
        wait_ms,
        max_retries,
    )
    .await
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
) -> Result<NetworkResult, Error> {
    query_with_optional_params(pool, "cloudsync_network_sync", wait_ms, max_retries).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_sync_result() {
        let result: NetworkResult = serde_json::from_str(
            r#"{
                "send": {
                    "status": "synced",
                    "localVersion": 12,
                    "serverVersion": 12,
                    "lastFailure": {"message": "previous apply failed"}
                },
                "receive": {
                    "rows": 3,
                    "tables": ["sessions", "notes"],
                    "error": "schema hash mismatch",
                    "lastFailure": {"message": "previous check failed"}
                }
            }"#,
        )
        .unwrap();

        assert_eq!(result.send.as_ref().unwrap().status, "synced");
        assert_eq!(result.send.as_ref().unwrap().local_version, 12);
        assert_eq!(result.receive.as_ref().unwrap().rows, 3);
        assert_eq!(
            result.receive.as_ref().unwrap().tables,
            ["sessions", "notes"]
        );
        assert_eq!(
            result.receive.as_ref().unwrap().error.as_deref(),
            Some("schema hash mismatch")
        );
    }

    #[test]
    fn parses_scoped_network_results() {
        let send: NetworkResult = serde_json::from_str(
            r#"{"send":{"status":"syncing","localVersion":8,"serverVersion":7}}"#,
        )
        .unwrap();
        let receive: NetworkResult =
            serde_json::from_str(r#"{"receive":{"rows":2,"tables":["sessions"]}}"#).unwrap();

        assert!(send.send.is_some());
        assert!(send.receive.is_none());
        assert!(receive.send.is_none());
        assert_eq!(receive.receive.unwrap().rows, 2);
    }
}

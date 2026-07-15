use serde::{Deserialize, Serialize};
use sqlx::{Executor, Sqlite};

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
    #[serde(default)]
    pub chunks: i64,
    #[serde(default)]
    pub bytes: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failure: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkReceiveResult {
    pub rows: i64,
    pub tables: Vec<String>,
    #[serde(default)]
    pub chunks: i64,
    #[serde(default)]
    pub bytes: i64,
    #[serde(default)]
    pub complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failure: Option<serde_json::Value>,
}

async fn query_with_optional_params<'e, E>(
    executor: E,
    fn_name: &str,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<NetworkResult, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let response: String = match (wait_ms, max_retries) {
        (None, None) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {fn_name}()")))
                .fetch_one(executor)
                .await?
        }
        (Some(wait_ms), None) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {fn_name}(?)")))
                .bind(wait_ms)
                .fetch_one(executor)
                .await?
        }
        (None, Some(max_retries)) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {fn_name}(NULL, ?)")))
                .bind(max_retries)
                .fetch_one(executor)
                .await?
        }
        (Some(wait_ms), Some(max_retries)) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {fn_name}(?, ?)")))
                .bind(wait_ms)
                .bind(max_retries)
                .fetch_one(executor)
                .await?
        }
    };

    Ok(serde_json::from_str(&response)?)
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-init
pub async fn network_init<'e, E>(executor: E, connection_string: &str) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT cloudsync_network_init(?)")
        .bind(connection_string)
        .fetch_optional(executor)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-set-apikey
pub async fn network_set_apikey<'e, E>(executor: E, api_key: &str) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT cloudsync_network_set_apikey(?)")
        .bind(api_key)
        .fetch_optional(executor)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-set-token
pub async fn network_set_token<'e, E>(executor: E, token: &str) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT cloudsync_network_set_token(?)")
        .bind(token)
        .fetch_optional(executor)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-cleanup
pub async fn network_cleanup<'e, E>(executor: E) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT cloudsync_network_cleanup()")
        .fetch_optional(executor)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-has-unsent-changes
pub async fn network_has_unsent_changes<'e, E>(executor: E) -> Result<bool, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    Ok(
        sqlx::query_scalar("SELECT cloudsync_network_has_unsent_changes()")
            .fetch_one(executor)
            .await?,
    )
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-send-changes
pub async fn network_send_changes<'e, E>(executor: E) -> Result<NetworkResult, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let response: String = sqlx::query_scalar("SELECT cloudsync_network_send_changes()")
        .fetch_one(executor)
        .await?;

    Ok(serde_json::from_str(&response)?)
}

pub async fn network_receive_changes<'e, E>(
    executor: E,
    max_chunks: Option<i64>,
) -> Result<NetworkResult, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let response: String = match max_chunks {
        Some(max_chunks) => {
            sqlx::query_scalar("SELECT cloudsync_network_receive_changes(?)")
                .bind(max_chunks)
                .fetch_one(executor)
                .await?
        }
        None => {
            sqlx::query_scalar("SELECT cloudsync_network_receive_changes()")
                .fetch_one(executor)
                .await?
        }
    };

    Ok(serde_json::from_str(&response)?)
}

/// Deprecated alias for [`network_receive_changes`].
pub async fn network_check_changes<'e, E>(
    executor: E,
    max_chunks: Option<i64>,
) -> Result<NetworkResult, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    network_receive_changes(executor, max_chunks).await
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-reset-sync-version
pub async fn network_reset_sync_version<'e, E>(executor: E) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT cloudsync_network_reset_sync_version()")
        .fetch_optional(executor)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-logout
pub async fn network_logout<'e, E>(executor: E) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT cloudsync_network_logout()")
        .fetch_optional(executor)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-sync
pub async fn network_sync<'e, E>(
    executor: E,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<NetworkResult, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    query_with_optional_params(executor, "cloudsync_network_sync", wait_ms, max_retries).await
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
                    "chunks": 3,
                    "bytes": 7340032,
                    "lastFailure": {"message": "previous apply failed"}
                },
                "receive": {
                    "rows": 3,
                    "tables": ["sessions", "notes"],
                    "chunks": 2,
                    "bytes": 4096,
                    "complete": true,
                    "error": "schema hash mismatch",
                    "lastFailure": {"message": "previous check failed"}
                }
            }"#,
        )
        .unwrap();

        assert_eq!(result.send.as_ref().unwrap().status, "synced");
        assert_eq!(result.send.as_ref().unwrap().local_version, 12);
        assert_eq!(result.send.as_ref().unwrap().chunks, 3);
        assert_eq!(result.send.as_ref().unwrap().bytes, 7_340_032);
        assert_eq!(result.receive.as_ref().unwrap().rows, 3);
        assert_eq!(result.receive.as_ref().unwrap().chunks, 2);
        assert_eq!(result.receive.as_ref().unwrap().bytes, 4096);
        assert!(result.receive.as_ref().unwrap().complete);
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

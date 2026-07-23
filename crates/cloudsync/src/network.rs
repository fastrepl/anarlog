use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use sqlx::{Connection, Executor, Sqlite, SqliteConnection};

use crate::error::Error;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingPayloadBatch {
    pub start_db_version: i64,
    pub watermark_db_version: Option<i64>,
    pub chunks: u32,
    pub rows: u64,
    pub bytes: u64,
    pub complete: bool,
    pub fits: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub last_optimistic_version: i64,
    pub last_confirmed_version: i64,
    pub gaps: Vec<serde_json::Value>,
    #[serde(default)]
    pub failures: NetworkStatusFailures,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetworkStatusFields {
    #[serde(default)]
    last_optimistic_version: Option<i64>,
    last_confirmed_version: i64,
    gaps: serde_json::Value,
    #[serde(default)]
    failures: Option<NetworkStatusFailures>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum NetworkStatusResponse {
    Envelope { data: NetworkStatusFields },
    Direct(NetworkStatusFields),
}

impl<'de> Deserialize<'de> for NetworkStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let response = NetworkStatusResponse::deserialize(deserializer)?;
        let response = match response {
            NetworkStatusResponse::Envelope { data } => data,
            NetworkStatusResponse::Direct(response) => response,
        };
        let last_confirmed_version = response.last_confirmed_version;
        let last_optimistic_version = response
            .last_optimistic_version
            .unwrap_or(last_confirmed_version);
        if last_optimistic_version < 0 || last_confirmed_version < 0 {
            return Err(serde::de::Error::custom(
                "cloudsync network versions must be non-negative",
            ));
        }
        let gaps = match response.gaps {
            serde_json::Value::Null => Vec::new(),
            serde_json::Value::Array(gaps) => gaps,
            _ => {
                return Err(serde::de::Error::custom(
                    "cloudsync network gaps must be an array or null",
                ));
            }
        };
        Ok(Self {
            // Confirmed progress is a conservative lower bound for optimistic
            // progress; some status responses omit the optimistic checkpoint.
            last_optimistic_version,
            last_confirmed_version,
            gaps,
            failures: response.failures.unwrap_or_default(),
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NetworkStatusFailures {
    #[serde(default)]
    pub apply: Option<serde_json::Value>,
    #[serde(default)]
    pub check: Option<serde_json::Value>,
}

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

pub async fn pending_payload_batch(
    connection: &mut SqliteConnection,
    max_chunks: u32,
    max_rows: u64,
    max_bytes: u64,
) -> Result<PendingPayloadBatch, Error> {
    if max_chunks == 0 || max_rows == 0 || max_bytes == 0 {
        return Err(Error::InvalidPendingPayloadLimits);
    }

    let start_db_version: i64 = sqlx::query_scalar(
        "SELECT COALESCE(
            (
                SELECT CAST(value AS INTEGER)
                FROM cloudsync_settings
                WHERE key = 'send_dbversion'
            ),
            0
        )",
    )
    .fetch_one(&mut *connection)
    .await?;
    let row_limit = i64::from(max_chunks) + 1;
    let mut chunks = sqlx::query_as::<_, (i64, i64, i64, bool)>(
        "SELECT payload_size, rows, watermark_db_version, is_final
         FROM cloudsync_payload_chunks
         LIMIT ?",
    )
    .bind(row_limit)
    .fetch(&mut *connection);
    let mut batch = PendingPayloadBatch {
        start_db_version,
        complete: true,
        fits: true,
        ..Default::default()
    };
    let mut saw_chunk = false;

    while let Some((payload_size, rows, watermark_db_version, is_final)) = chunks.try_next().await?
    {
        saw_chunk = true;
        let payload_size = u64::try_from(payload_size).map_err(|_| {
            std::io::Error::other("cloudsync pending payload scan returned a negative payload size")
        })?;
        let rows = u64::try_from(rows).map_err(|_| {
            std::io::Error::other("cloudsync pending payload scan returned a negative row count")
        })?;
        batch.chunks = batch.chunks.saturating_add(1);
        batch.rows = batch.rows.saturating_add(rows);
        batch.bytes = batch.bytes.saturating_add(payload_size);
        batch.complete = is_final;
        match batch.watermark_db_version {
            Some(watermark) if watermark != watermark_db_version => {
                return Err(std::io::Error::other(
                    "cloudsync pending payload scan returned inconsistent watermarks",
                )
                .into());
            }
            None => batch.watermark_db_version = Some(watermark_db_version),
            _ => {}
        }

        if batch.chunks > max_chunks || batch.rows > max_rows || batch.bytes > max_bytes {
            batch.fits = false;
            return Ok(batch);
        }

        if is_final {
            return Ok(batch);
        }
    }

    if saw_chunk {
        batch.complete = false;
        batch.fits = false;
    }
    Ok(batch)
}

pub async fn network_status<'e, E>(executor: E) -> Result<NetworkStatus, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let response: String = sqlx::query_scalar("SELECT cloudsync_network_status()")
        .fetch_one(executor)
        .await?;

    Ok(serde_json::from_str(&response)?)
}

pub async fn reconcile_confirmed_pending_payload(
    connection: &mut SqliteConnection,
    batch: PendingPayloadBatch,
    status: &NetworkStatus,
) -> Result<bool, Error> {
    let Some(watermark_db_version) = batch.watermark_db_version else {
        return Ok(false);
    };
    if batch.chunks == 0
        || watermark_db_version <= batch.start_db_version
        || !batch.complete
        || !batch.fits
        || !status.gaps.is_empty()
        || status.failures.apply.is_some()
        || status.last_optimistic_version < watermark_db_version
        || status.last_confirmed_version < watermark_db_version
    {
        return Ok(false);
    }

    let mut transaction = connection.begin().await?;
    let current_db_version: i64 = sqlx::query_scalar(
        "SELECT COALESCE(
            (
                SELECT CAST(value AS INTEGER)
                FROM cloudsync_settings
                WHERE key = 'send_dbversion'
            ),
            0
        )",
    )
    .fetch_one(&mut *transaction)
    .await?;
    if current_db_version != batch.start_db_version {
        transaction.rollback().await?;
        return Ok(false);
    }

    sqlx::query("SELECT cloudsync_set('send_dbversion', CAST(? AS TEXT))")
        .bind(watermark_db_version)
        .fetch_optional(&mut *transaction)
        .await?;
    let updated_db_version: i64 = sqlx::query_scalar(
        "SELECT CAST(value AS INTEGER)
         FROM cloudsync_settings
         WHERE key = 'send_dbversion'",
    )
    .fetch_one(&mut *transaction)
    .await?;
    if updated_db_version != watermark_db_version {
        transaction.rollback().await?;
        return Err(
            std::io::Error::other("cloudsync send cursor reconciliation did not persist").into(),
        );
    }

    transaction.commit().await?;
    Ok(true)
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

pub async fn network_reset_receive_version(connection: &mut SqliteConnection) -> Result<(), Error> {
    let mut transaction = connection.begin().await?;
    let before = read_network_cursors(&mut transaction).await?;

    // sqlite-sync 1.1.2's public reset zeros all four cursors, so a receive-only
    // full resync must set the two durable check cursors directly.
    sqlx::query(
        "SELECT
            cloudsync_set('check_dbversion', '0'),
            cloudsync_set('check_seq', '0')",
    )
    .fetch_optional(&mut *transaction)
    .await?;

    let after = read_network_cursors(&mut transaction).await?;
    let verification_error = if after.0.as_deref() != Some("0") || after.1.as_deref() != Some("0") {
        Some("receive cursor write did not persist")
    } else if after.2 != before.2 || after.3 != before.3 {
        Some("outbound send cursor changed")
    } else {
        None
    };

    if let Some(message) = verification_error {
        transaction.rollback().await?;
        return Err(std::io::Error::other(format!(
            "cloudsync receive cursor reset verification failed: {message}"
        ))
        .into());
    }

    transaction.commit().await?;
    Ok(())
}

async fn read_network_cursors(
    connection: &mut SqliteConnection,
) -> Result<
    (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ),
    Error,
> {
    Ok(sqlx::query_as(
        "SELECT
            MAX(CASE WHEN key = 'check_dbversion' THEN value END),
            MAX(CASE WHEN key = 'check_seq' THEN value END),
            MAX(CASE WHEN key = 'send_dbversion' THEN value END),
            MAX(CASE WHEN key = 'send_seq' THEN value END)
         FROM cloudsync_settings",
    )
    .fetch_one(connection)
    .await?)
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

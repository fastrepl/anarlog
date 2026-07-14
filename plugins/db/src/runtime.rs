use std::path::Path;

use hypr_db_core::{Db, DbOpenOptions, DbStorage};
use hypr_db_execute::{DbExecutor, ProxyQueryMethod, ProxyQueryResult};
use hypr_db_reactive::{LiveQueryRuntime, QueryEventSink, SubscriptionRegistration};
use tauri::ipc::Channel;

use crate::{QueryEvent, Result, TransactionStatement};

const DEFAULT_CLOUDSYNC_INTERVAL_MS: u64 = 30_000;

#[derive(Clone)]
pub struct QueryEventChannel(Channel<QueryEvent>);

impl QueryEventChannel {
    pub fn new(channel: Channel<QueryEvent>) -> Self {
        Self(channel)
    }
}

impl QueryEventSink for QueryEventChannel {
    fn send_result(&self, rows: Vec<serde_json::Value>) -> std::result::Result<(), String> {
        self.0
            .send(QueryEvent::Result(rows))
            .map_err(|error| error.to_string())
    }

    fn send_error(&self, error: String) -> std::result::Result<(), String> {
        self.0
            .send(QueryEvent::Error(error))
            .map_err(|error| error.to_string())
    }
}

pub struct PluginDbRuntime {
    db: std::sync::Arc<Db>,
    schema_ready: tokio::sync::OnceCell<()>,
    executor: DbExecutor,
    live_query_runtime: LiveQueryRuntime<QueryEventChannel>,
}

impl PluginDbRuntime {
    pub fn new(db: std::sync::Arc<Db>) -> Self {
        Self {
            db: std::sync::Arc::clone(&db),
            schema_ready: tokio::sync::OnceCell::new(),
            executor: DbExecutor::new(std::sync::Arc::clone(&db)),
            live_query_runtime: LiveQueryRuntime::new(db),
        }
    }

    pub fn pool(&self) -> &sqlx::SqlitePool {
        self.db.pool()
    }

    async fn ensure_app_schema(&self) -> Result<()> {
        self.schema_ready
            .get_or_try_init(|| async { hypr_db_app::prepare_schema(self.db.as_ref()).await })
            .await?;
        Ok(())
    }

    pub async fn execute(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>> {
        self.ensure_app_schema().await?;
        Ok(self.executor.execute(sql, params).await?)
    }

    pub async fn execute_transaction(
        &self,
        statements: Vec<TransactionStatement>,
    ) -> Result<Vec<u64>> {
        self.ensure_app_schema().await?;
        let mut transaction = self.db.pool().begin_with("BEGIN IMMEDIATE").await?;
        let mut rows_affected = Vec::with_capacity(statements.len());

        for (statement_index, statement) in statements.into_iter().enumerate() {
            let result = bind_params(
                sqlx::query(sqlx::AssertSqlSafe(statement.sql.as_str())),
                &statement.params,
            )
            .execute(&mut *transaction)
            .await?;
            let actual = result.rows_affected();
            if let Some(expected) = statement.expected_rows_affected
                && actual != expected
            {
                return Err(crate::Error::UnexpectedRowsAffected {
                    statement_index,
                    expected,
                    actual,
                });
            }
            rows_affected.push(actual);
        }

        transaction.commit().await?;
        Ok(rows_affected)
    }

    pub async fn execute_proxy(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
        method: ProxyQueryMethod,
    ) -> Result<ProxyQueryResult> {
        self.ensure_app_schema().await?;
        Ok(self.executor.execute_proxy(sql, params, method).await?)
    }

    pub async fn subscribe(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
        sink: QueryEventChannel,
    ) -> Result<SubscriptionRegistration> {
        self.ensure_app_schema().await?;
        Ok(self.live_query_runtime.subscribe(sql, params, sink).await?)
    }

    pub async fn unsubscribe(&self, subscription_id: &str) -> hypr_db_reactive::Result<()> {
        self.live_query_runtime.unsubscribe(subscription_id).await
    }

    pub async fn configure_cloudsync(&self, config_json: String) -> Result<()> {
        let config = serde_json::from_str(&config_json)?;
        self.db.cloudsync_configure(config).await?;
        Ok(())
    }

    pub async fn configure_cloudsync_token(
        &self,
        database_id: String,
        token: String,
        workspace_id: String,
    ) -> Result<bool> {
        if !self.claim_cloudsync_account(workspace_id).await? {
            return Ok(false);
        }

        self.apply_cloudsync_config_fail_closed(hypr_db_core::CloudsyncRuntimeConfig {
            connection_string: database_id,
            auth: hypr_db_core::CloudsyncAuth::Token { token },
            tables: hypr_db_app::cloudsync_table_registry().to_vec(),
            sync_interval_ms: DEFAULT_CLOUDSYNC_INTERVAL_MS,
            wait_ms: Some(5_000),
            max_retries: Some(3),
        })
        .await?;
        Ok(true)
    }

    pub async fn claim_cloudsync_account(&self, account_user_id: String) -> Result<bool> {
        self.ensure_app_schema().await?;
        self.db.cloudsync_suspend().await?;
        match hypr_db_app::claim_cloudsync_workspace(self.db.pool(), &account_user_id).await {
            Ok(()) => Ok(true),
            Err(hypr_db_app::CloudsyncWorkspaceError::AccountMismatch) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    async fn apply_cloudsync_config_fail_closed(
        &self,
        config: hypr_db_core::CloudsyncRuntimeConfig,
    ) -> Result<()> {
        let result = async {
            self.db.cloudsync_reconfigure(config).await?;
            self.db.cloudsync_start().await
        }
        .await;

        if let Err(error) = result {
            let _ = self.db.cloudsync_suspend().await;
            return Err(error.into());
        }

        Ok(())
    }

    pub async fn start_cloudsync(&self) -> Result<()> {
        self.ensure_app_schema().await?;
        self.db.cloudsync_start().await?;
        Ok(())
    }

    pub async fn stop_cloudsync(&self) -> Result<()> {
        self.db.cloudsync_stop().await?;
        Ok(())
    }

    pub async fn suspend_cloudsync(&self) -> Result<()> {
        self.db.cloudsync_suspend().await?;
        Ok(())
    }

    pub async fn cloudsync_status(&self) -> Result<serde_json::Value> {
        Ok(serde_json::to_value(self.db.cloudsync_status().await?)?)
    }

    pub async fn sync_cloudsync_now(&self) -> Result<serde_json::Value> {
        Ok(serde_json::to_value(
            self.db.cloudsync_trigger_sync().await?,
        )?)
    }

    pub async fn logout_cloudsync(&self, discard_unsent_changes: bool) -> Result<()> {
        self.db.cloudsync_logout(discard_unsent_changes).await?;
        Ok(())
    }
}

fn bind_params<'q>(
    mut query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments>,
    params: &[serde_json::Value],
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments> {
    for param in params {
        query = match param {
            serde_json::Value::Null => query.bind(None::<String>),
            serde_json::Value::Bool(value) => query.bind(*value),
            serde_json::Value::Number(value) => {
                if let Some(integer) = value.as_i64() {
                    query.bind(integer)
                } else {
                    query.bind(value.as_f64().unwrap_or_default())
                }
            }
            serde_json::Value::String(value) => query.bind(value.clone()),
            other => query.bind(other.to_string()),
        };
    }

    query
}

pub async fn open_app_db(db_path: Option<&Path>) -> Result<Db> {
    let storage = match db_path {
        Some(path) => DbStorage::Local(path),
        None => DbStorage::Memory,
    };

    let db = Db::open(DbOpenOptions {
        storage,
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await?;

    hypr_db_app::prepare_schema(&db).await?;

    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn failed_cloudsync_start_clears_new_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = Db::open(DbOpenOptions {
            storage: DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(4),
        })
        .await
        .unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let runtime = PluginDbRuntime::new(std::sync::Arc::new(db));

        runtime
            .apply_cloudsync_config_fail_closed(hypr_db_core::CloudsyncRuntimeConfig {
                connection_string: "managed-database-id".to_string(),
                auth: hypr_db_core::CloudsyncAuth::Token {
                    token: "secret-token".to_string(),
                },
                tables: vec![hypr_db_core::CloudsyncTableSpec {
                    table_name: "missing_table".to_string(),
                    crdt_algo: None,
                    init_flags: None,
                    enabled: true,
                }],
                sync_interval_ms: DEFAULT_CLOUDSYNC_INTERVAL_MS,
                wait_ms: Some(5_000),
                max_retries: Some(3),
            })
            .await
            .unwrap_err();

        let status = runtime.cloudsync_status().await.unwrap();
        assert_eq!(status["configured"], false);
        assert_eq!(status["running"], false);
        assert_eq!(status["network_initialized"], false);
    }
}

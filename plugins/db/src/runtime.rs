use std::collections::HashMap;
use std::path::Path;

use hypr_db_core::{Db, DbOpenError, DbOpenOptions, DbStorage};
use hypr_db_execute::{DbExecutor, ProxyQueryMethod, ProxyQueryResult};
use hypr_db_reactive::{LiveQueryRuntime, QueryEventSink, SubscriptionRegistration};
use tauri::ipc::Channel;

use crate::{QueryEvent, Result, TransactionStatement};

const DEFAULT_CLOUDSYNC_INTERVAL_MS: u64 = 30_000;
const CLOUDSYNC_WRITE_FILTER: &str =
    "workspace_id IN (SELECT allowed_workspace_id FROM cloudsync_writable_workspaces)";

#[derive(Default)]
struct E2eeSyncHook {
    keys: std::sync::RwLock<HashMap<String, hypr_e2ee::WorkspaceKey>>,
    witness: std::sync::RwLock<Option<crate::e2ee_witness::E2eeWitnessClient>>,
}

impl E2eeSyncHook {
    fn set_personal_workspace(
        &self,
        workspace_id: &str,
        recovery_key: &hypr_e2ee::RecoveryKey,
    ) -> std::result::Result<(), hypr_e2ee::Error> {
        let key = recovery_key.workspace_key(workspace_id)?;
        *self.keys.write().unwrap() = HashMap::from([(workspace_id.to_string(), key)]);
        Ok(())
    }

    fn has_workspace(&self, workspace_id: &str) -> bool {
        self.keys.read().unwrap().contains_key(workspace_id)
    }

    fn workspace_key(&self, workspace_id: &str) -> Option<hypr_e2ee::WorkspaceKey> {
        self.keys.read().unwrap().get(workspace_id).cloned()
    }

    fn clear(&self) {
        self.keys.write().unwrap().clear();
        *self.witness.write().unwrap() = None;
    }

    fn snapshot(&self) -> HashMap<String, hypr_e2ee::WorkspaceKey> {
        self.keys.read().unwrap().clone()
    }

    fn set_witness(&self, witness: crate::e2ee_witness::E2eeWitnessClient) {
        *self.witness.write().unwrap() = Some(witness);
    }

    fn witness(&self) -> Option<crate::e2ee_witness::E2eeWitnessClient> {
        self.witness.read().unwrap().clone()
    }

    async fn prepare_local_snapshot(
        &self,
        pool: &sqlx::SqlitePool,
    ) -> std::result::Result<(), hypr_db_app::E2eeReplicaError> {
        hypr_db_app::encrypt_e2ee_replica_changes(pool, &self.snapshot())
            .await
            .map(|_| ())
    }
}

impl hypr_db_core::CloudsyncSyncHook for E2eeSyncHook {
    fn before_sync<'a>(
        &'a self,
        pool: &'a sqlx::SqlitePool,
    ) -> hypr_db_core::CloudsyncHookFuture<'a> {
        let keys = self.snapshot();
        let witness = self.witness();
        Box::pin(async move {
            let witness = witness
                .ok_or_else(|| std::io::Error::other("E2EE freshness witness is not configured"))?;
            let key = keys.get(witness.workspace_id()).ok_or_else(|| {
                std::io::Error::other("E2EE freshness witness identity is not configured")
            })?;
            let stats = hypr_db_app::encrypt_e2ee_replica_changes(pool, &keys)
                .await
                .map_err(|error| {
                    std::io::Error::other(format!("E2EE pre-sync encryption failed: {error}"))
                })?;
            tracing::debug!(
                encrypted_fields = stats.encrypted_fields,
                "prepared encrypted CloudSync replica"
            );
            witness.publish_and_refresh(pool, key).await?;
            let stats = hypr_db_app::apply_e2ee_replica_changes_with_witness(pool, &keys)
                .await
                .map_err(|error| {
                    std::io::Error::other(format!("E2EE pre-sync witness apply failed: {error}"))
                })?;
            tracing::debug!(
                applied_fields = stats.applied_fields,
                rejected_unwitnessed = stats.rejected_unwitnessed,
                "applied trusted E2EE witness before CloudSync"
            );
            Ok(())
        })
    }

    fn after_sync<'a>(
        &'a self,
        pool: &'a sqlx::SqlitePool,
    ) -> hypr_db_core::CloudsyncHookFuture<'a> {
        let keys = self.snapshot();
        let witness = self.witness();
        Box::pin(async move {
            let witness = witness
                .ok_or_else(|| std::io::Error::other("E2EE freshness witness is not configured"))?;
            let key = keys.get(witness.workspace_id()).ok_or_else(|| {
                std::io::Error::other("E2EE freshness witness identity is not configured")
            })?;
            witness.refresh(pool, key).await?;
            let stats = hypr_db_app::apply_e2ee_replica_changes_with_witness(pool, &keys)
                .await
                .map_err(|error| {
                    std::io::Error::other(format!("E2EE post-sync decryption failed: {error}"))
                })?;
            tracing::debug!(
                applied_fields = stats.applied_fields,
                skipped_local_changes = stats.skipped_local_changes,
                rejected_rollbacks = stats.rejected_rollbacks,
                rejected_unwitnessed = stats.rejected_unwitnessed,
                "applied encrypted CloudSync replica"
            );
            Ok(())
        })
    }
}

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
    synced_write_barrier: tokio::sync::RwLock<()>,
    executor: DbExecutor,
    live_query_runtime: LiveQueryRuntime<QueryEventChannel>,
    e2ee_sync_hook: std::sync::Arc<E2eeSyncHook>,
}

impl PluginDbRuntime {
    pub fn new(db: std::sync::Arc<Db>) -> Self {
        let e2ee_sync_hook = std::sync::Arc::new(E2eeSyncHook::default());
        db.set_cloudsync_sync_hook(e2ee_sync_hook.clone());
        Self {
            db: std::sync::Arc::clone(&db),
            schema_ready: tokio::sync::OnceCell::new(),
            synced_write_barrier: tokio::sync::RwLock::new(()),
            executor: DbExecutor::new(std::sync::Arc::clone(&db)),
            live_query_runtime: LiveQueryRuntime::new(db),
            e2ee_sync_hook,
        }
    }

    pub fn set_e2ee_recovery_key(
        &self,
        workspace_id: &str,
        recovery_key: &hypr_e2ee::RecoveryKey,
    ) -> Result<()> {
        self.e2ee_sync_hook
            .set_personal_workspace(workspace_id, recovery_key)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        Ok(())
    }

    pub fn pool(&self) -> &sqlx::SqlitePool {
        self.db.pool()
    }

    pub fn workspace_key(&self, workspace_id: &str) -> Option<hypr_e2ee::WorkspaceKey> {
        self.e2ee_sync_hook.workspace_key(workspace_id)
    }

    pub async fn synced_write_guard(&self) -> tokio::sync::RwLockReadGuard<'_, ()> {
        self.synced_write_barrier.read().await
    }

    async fn ensure_app_schema(&self) -> Result<()> {
        self.schema_ready
            .get_or_try_init(|| async { hypr_db_app::prepare_schema(self.db.as_ref()).await })
            .await?;
        Ok(())
    }

    async fn ensure_legacy_migration_verified(&self) -> Result<()> {
        self.ensure_app_schema().await?;
        if crate::import::legacy_migration_verified(self.db.pool()).await? {
            return Ok(());
        }

        let _ = self.db.cloudsync_suspend().await;
        Err(std::io::Error::other(
            "legacy data migration needs attention before CloudSync can start",
        )
        .into())
    }

    pub async fn execute(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>> {
        let _write_guard = self.synced_write_barrier.read().await;
        self.ensure_app_schema().await?;
        Ok(self.executor.execute(sql, params).await?)
    }

    pub async fn execute_transaction(
        &self,
        statements: Vec<TransactionStatement>,
    ) -> Result<Vec<u64>> {
        let _write_guard = self.synced_write_barrier.read().await;
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
        let _write_guard = self.synced_write_barrier.read().await;
        self.ensure_app_schema().await?;
        Ok(self.executor.execute_proxy(sql, params, method).await?)
    }

    pub async fn cleanup_legacy_files(&self) -> Result<crate::LegacyCleanupResult> {
        let _write_guard = self.synced_write_barrier.read().await;
        Ok(crate::import::cleanup_legacy_files(self.db.pool()).await?)
    }

    pub async fn rerun_legacy_import(&self, dry_run: bool) -> Result<String> {
        let _write_guard = self.synced_write_barrier.read().await;
        Ok(crate::import::rerun_legacy_import(self.db.pool(), dry_run).await?)
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
        self.ensure_legacy_migration_verified().await?;
        let config = serde_json::from_str(&config_json)?;
        self.db.cloudsync_configure(config).await?;
        Ok(())
    }

    pub async fn configure_cloudsync_token(
        &self,
        database_id: String,
        token: String,
        account_user_id: String,
        e2ee_witness: crate::CloudsyncE2eeWitness,
    ) -> Result<crate::CloudsyncTokenConfigurationResult> {
        self.configure_cloudsync_token_with_projection(
            database_id,
            token,
            account_user_id,
            None,
            e2ee_witness,
        )
        .await
    }

    pub async fn configure_cloudsync_token_with_projection(
        &self,
        database_id: String,
        token: String,
        account_user_id: String,
        workspace_projection: Option<hypr_db_app::CloudsyncWorkspaceProjection>,
        e2ee_witness: crate::CloudsyncE2eeWitness,
    ) -> Result<crate::CloudsyncTokenConfigurationResult> {
        let result = self
            .configure_cloudsync_token_with_projection_inner(
                database_id,
                token,
                account_user_id,
                workspace_projection,
                e2ee_witness,
            )
            .await;
        if result.is_err()
            || matches!(
                &result,
                Ok(crate::CloudsyncTokenConfigurationResult::AccountMismatch)
            )
        {
            let _ = self.db.cloudsync_suspend().await;
            self.e2ee_sync_hook.clear();
        }
        result
    }

    async fn configure_cloudsync_token_with_projection_inner(
        &self,
        database_id: String,
        token: String,
        account_user_id: String,
        workspace_projection: Option<hypr_db_app::CloudsyncWorkspaceProjection>,
        e2ee_witness: crate::CloudsyncE2eeWitness,
    ) -> Result<crate::CloudsyncTokenConfigurationResult> {
        if !self.db.cloudsync_enabled() {
            return Err(hypr_db_core::CloudsyncRuntimeError::Unavailable.into());
        }

        if workspace_projection
            .as_ref()
            .is_some_and(|projection| projection.account_user_id != account_user_id)
        {
            return Err(hypr_db_app::CloudsyncWorkspaceError::InvalidWorkspaceProjection.into());
        }
        if let Some(projection) = workspace_projection.as_ref() {
            hypr_db_app::validate_cloudsync_workspace_projection(projection)?;
        }

        self.ensure_legacy_migration_verified().await?;

        let personal_workspace_id = workspace_projection
            .as_ref()
            .map(|projection| projection.personal_workspace_id.as_str())
            .unwrap_or(account_user_id.as_str());
        if personal_workspace_id != account_user_id
            || !self.e2ee_sync_hook.has_workspace(personal_workspace_id)
        {
            let _ = self.db.cloudsync_suspend().await;
            return Err(crate::Error::E2eeIdentityRequired);
        }

        if !self
            .claim_cloudsync_workspace(account_user_id.clone())
            .await?
        {
            return Ok(crate::CloudsyncTokenConfigurationResult::AccountMismatch);
        }

        if workspace_projection.is_some() {
            self.db.cloudsync_suspend().await?;
        }
        let reconciliation_guard = if workspace_projection.is_some() {
            Some(self.synced_write_barrier.write().await)
        } else {
            None
        };
        self.prepare_e2ee_cutover().await?;
        let witness =
            crate::e2ee_witness::E2eeWitnessClient::new(e2ee_witness, personal_workspace_id)?;
        let key = self
            .e2ee_sync_hook
            .workspace_key(personal_workspace_id)
            .ok_or(crate::Error::E2eeIdentityRequired)?;
        witness.initialize(self.db.pool(), &key).await?;
        self.e2ee_sync_hook.set_witness(witness);

        let write_filter_version_current = match workspace_projection.as_ref() {
            Some(_) => hypr_db_app::cloudsync_write_filter_version_current(self.db.pool()).await?,
            None => true,
        };
        let write_filter_installed = match workspace_projection.as_ref() {
            Some(projection) => {
                hypr_db_app::cloudsync_write_filter_installed(
                    self.db.pool(),
                    &projection.personal_workspace_id,
                )
                .await?
                    && self.cloudsync_write_filters_match().await?
            }
            None => true,
        };
        let write_filter_requires_reset = if write_filter_installed {
            false
        } else {
            write_filter_version_current || self.cloudsync_has_initialized_tables().await?
        };
        let mut install_write_filter = !write_filter_installed;

        let reconciliation = match workspace_projection.as_ref() {
            Some(projection) => Some(
                hypr_db_app::stage_cloudsync_workspace_reconciliation(self.db.pool(), projection)
                    .await?,
            ),
            None => None,
        };
        let config = hypr_db_core::CloudsyncRuntimeConfig {
            connection_string: database_id,
            auth: hypr_db_core::CloudsyncAuth::Token { token },
            tables: hypr_db_app::cloudsync_table_registry().to_vec(),
            sync_interval_ms: DEFAULT_CLOUDSYNC_INTERVAL_MS,
            wait_ms: Some(5_000),
            max_retries: Some(3),
        };

        if reconciliation
            .as_ref()
            .is_some_and(|plan| plan.requires_replica_reset())
            || write_filter_requires_reset
        {
            self.apply_cloudsync_config_fail_closed(config.clone())
                .await?;
            match self.db.cloudsync_trigger_sync().await {
                Ok(result) if cloudsync_send_completed(&result) => {}
                Ok(_) => {
                    let _ = self.db.cloudsync_suspend().await;
                    return Err(hypr_db_core::CloudsyncRuntimeError::UnsentChanges.into());
                }
                Err(error) => {
                    let _ = self.db.cloudsync_suspend().await;
                    return Err(error.into());
                }
            }
            let status = match self.db.cloudsync_status().await {
                Ok(status) => status,
                Err(error) => {
                    let _ = self.db.cloudsync_suspend().await;
                    return Err(error.into());
                }
            };
            if status.has_unsent_changes != Some(false) {
                let _ = self.db.cloudsync_suspend().await;
                return Err(hypr_db_core::CloudsyncRuntimeError::UnsentChanges.into());
            }
            if let Err(error) = self.db.cloudsync_logout(false).await {
                let _ = self.db.cloudsync_suspend().await;
                return Err(error.into());
            }
            install_write_filter = true;
        }

        if let Some(projection) = workspace_projection.as_ref()
            && install_write_filter
        {
            hypr_db_app::set_cloudsync_personal_write_scope(
                self.db.pool(),
                &projection.personal_workspace_id,
            )
            .await?;
            for table in hypr_db_app::cloudsync_table_registry()
                .iter()
                .filter(|table| table.enabled)
            {
                self.db
                    .cloudsync_init(
                        &table.table_name,
                        table.crdt_algo.as_deref(),
                        table.init_flags,
                    )
                    .await
                    .map_err(hypr_db_core::CloudsyncRuntimeError::from)?;
                self.db
                    .cloudsync_set_filter(&table.table_name, CLOUDSYNC_WRITE_FILTER)
                    .await
                    .map_err(hypr_db_core::CloudsyncRuntimeError::from)?;
            }
            hypr_db_app::mark_cloudsync_write_filter_installed(self.db.pool()).await?;
        }

        if let (Some(projection), Some(reconciliation)) =
            (workspace_projection.as_ref(), reconciliation.as_ref())
        {
            let _ = hypr_db_app::commit_cloudsync_workspace_projection(
                self.db.pool(),
                projection,
                reconciliation.requires_full_resync() || install_write_filter,
            )
            .await?;
        }
        drop(reconciliation_guard);

        self.apply_cloudsync_config_fail_closed(config).await?;
        if let Some(generation) =
            hypr_db_app::cloudsync_full_resync_generation(self.db.pool()).await?
        {
            if let Err(error) = self.db.cloudsync_network_reset_sync_version().await {
                let _ = self.db.cloudsync_suspend().await;
                return Err(hypr_db_core::CloudsyncRuntimeError::from(error).into());
            }
            self.schedule_cloudsync_full_resync(generation);
        }
        Ok(crate::CloudsyncTokenConfigurationResult::Configured)
    }

    pub async fn bind_cloudsync_account(&self, account_user_id: String) -> Result<bool> {
        let _write_guard = self.synced_write_barrier.write().await;
        self.ensure_app_schema().await?;
        match hypr_db_app::bind_cloudsync_account(self.db.pool(), &account_user_id).await {
            Ok(()) => Ok(true),
            Err(hypr_db_app::CloudsyncWorkspaceError::AccountMismatch) => {
                self.db.cloudsync_suspend().await?;
                Ok(false)
            }
            Err(error) => {
                let _ = self.db.cloudsync_suspend().await;
                Err(error.into())
            }
        }
    }

    async fn claim_cloudsync_workspace(&self, account_user_id: String) -> Result<bool> {
        self.ensure_app_schema().await?;
        match hypr_db_app::cloudsync_workspace_is_claimed_by(self.db.pool(), &account_user_id).await
        {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(error) => {
                let _ = self.db.cloudsync_suspend().await;
                if is_permanent_cloudsync_workspace_rejection(&error) {
                    return Ok(false);
                }
                return Err(error.into());
            }
        }

        self.db.cloudsync_suspend().await?;
        match hypr_db_app::claim_cloudsync_workspace(self.db.pool(), &account_user_id).await {
            Ok(()) => Ok(true),
            Err(error) if is_permanent_cloudsync_workspace_rejection(&error) => Ok(false),
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

    async fn prepare_e2ee_cutover(&self) -> Result<()> {
        self.e2ee_sync_hook
            .prepare_local_snapshot(self.db.pool())
            .await
            .map_err(|error| std::io::Error::other(error.to_string()))?;

        for table_name in hypr_db_app::E2EE_DOMAIN_TABLES {
            if hypr_db_core::cloudsync_is_enabled_on(self.db.pool(), table_name)
                .await
                .map_err(hypr_db_core::CloudsyncRuntimeError::from)?
            {
                self.db
                    .cloudsync_cleanup(table_name)
                    .await
                    .map_err(hypr_db_core::CloudsyncRuntimeError::from)?;
            }
        }
        Ok(())
    }

    fn schedule_cloudsync_full_resync(&self, generation: String) {
        let db = std::sync::Arc::clone(&self.db);
        tokio::spawn(async move {
            for attempt in 0..3 {
                match db.cloudsync_trigger_sync().await {
                    Ok(result) if cloudsync_snapshot_completed(&result) => {
                        if let Err(error) =
                            hypr_db_app::clear_cloudsync_full_resync_pending(db.pool(), &generation)
                                .await
                        {
                            tracing::warn!(%error, "failed to clear CloudSync full resync marker");
                        }
                        return;
                    }
                    Ok(_) if attempt < 2 => {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                    Ok(_) => {
                        tracing::warn!("CloudSync full resync remains incomplete");
                    }
                    Err(error) => {
                        tracing::warn!(%error, "CloudSync full resync remains pending");
                        return;
                    }
                }
            }
        });
    }

    async fn cloudsync_has_initialized_tables(&self) -> Result<bool> {
        for table in hypr_db_app::cloudsync_table_registry()
            .iter()
            .filter(|table| table.enabled)
        {
            if hypr_db_core::cloudsync_is_enabled_on(self.db.pool(), &table.table_name)
                .await
                .map_err(hypr_db_core::CloudsyncRuntimeError::from)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(crate) async fn cloudsync_write_filters_match(&self) -> Result<bool> {
        let settings_exist: bool = sqlx::query_scalar(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'cloudsync_table_settings'
             )",
        )
        .fetch_one(self.db.pool())
        .await?;
        if !settings_exist {
            return Ok(false);
        }

        for table in hypr_db_app::cloudsync_table_registry()
            .iter()
            .filter(|table| table.enabled)
        {
            let matches: bool = sqlx::query_scalar(
                "SELECT EXISTS(
                   SELECT 1
                   FROM cloudsync_table_settings
                   WHERE tbl_name = ? COLLATE NOCASE
                     AND col_name = '*'
                     AND key = 'filter'
                     AND value = ?
                 )",
            )
            .bind(&table.table_name)
            .bind(CLOUDSYNC_WRITE_FILTER)
            .fetch_one(self.db.pool())
            .await?;
            if !matches {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub async fn start_cloudsync(&self) -> Result<()> {
        self.ensure_legacy_migration_verified().await?;
        self.db.cloudsync_start().await?;
        Ok(())
    }

    pub async fn stop_cloudsync(&self) -> Result<()> {
        self.db.cloudsync_stop().await?;
        Ok(())
    }

    pub async fn suspend_cloudsync(&self) -> Result<()> {
        self.db.cloudsync_suspend().await?;
        self.e2ee_sync_hook.clear();
        Ok(())
    }

    pub async fn cloudsync_status(&self) -> Result<serde_json::Value> {
        Ok(serde_json::to_value(self.db.cloudsync_status().await?)?)
    }

    pub async fn sync_cloudsync_now(&self) -> Result<serde_json::Value> {
        self.ensure_legacy_migration_verified().await?;
        Ok(serde_json::to_value(
            self.db.cloudsync_trigger_sync().await?,
        )?)
    }

    pub async fn logout_cloudsync(&self, discard_unsent_changes: bool) -> Result<()> {
        let _write_guard = self.synced_write_barrier.write().await;
        self.db.cloudsync_logout(discard_unsent_changes).await?;
        self.e2ee_sync_hook.clear();
        Ok(())
    }
}

fn cloudsync_send_completed(result: &hypr_db_core::CloudsyncNetworkResult) -> bool {
    let Some(send) = result.send.as_ref() else {
        return false;
    };
    let receive_completed = result.receive.as_ref().is_none_or(|receive| {
        receive.complete && receive.error.is_none() && receive.last_failure.is_none()
    });
    send.status == "synced" && send.last_failure.is_none() && receive_completed
}

fn cloudsync_snapshot_completed(result: &hypr_db_core::CloudsyncNetworkResult) -> bool {
    let Some(receive) = result.receive.as_ref() else {
        return false;
    };
    cloudsync_send_completed(result)
        && receive.complete
        && receive.error.is_none()
        && receive.last_failure.is_none()
}

fn is_permanent_cloudsync_workspace_rejection(
    error: &hypr_db_app::CloudsyncWorkspaceError,
) -> bool {
    matches!(
        error,
        hypr_db_app::CloudsyncWorkspaceError::InvalidWorkspaceId
            | hypr_db_app::CloudsyncWorkspaceError::InvalidBinding
            | hypr_db_app::CloudsyncWorkspaceError::AccountMismatch
            | hypr_db_app::CloudsyncWorkspaceError::ForeignWorkspace { .. }
    )
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

    match Db::open(app_db_open_options(storage, true)).await {
        Ok(db) => {
            hypr_db_app::prepare_schema(&db).await?;
            Ok(db)
        }
        Err(cloudsync_error) => {
            let probe_error = match probe_cloudsync_extension().await {
                Ok(()) => return Err(cloudsync_error.into()),
                Err(error) => error,
            };
            open_app_db_without_cloudsync(storage, cloudsync_error, probe_error).await
        }
    }
}

fn app_db_open_options(storage: DbStorage<'_>, cloudsync_enabled: bool) -> DbOpenOptions<'_> {
    DbOpenOptions {
        storage,
        cloudsync_enabled,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    }
}

async fn probe_cloudsync_extension() -> std::result::Result<(), DbOpenError> {
    let db = Db::open(app_db_open_options(DbStorage::Memory, true)).await?;
    db.pool().close().await;
    Ok(())
}

async fn open_app_db_without_cloudsync(
    storage: DbStorage<'_>,
    cloudsync_error: DbOpenError,
    probe_error: DbOpenError,
) -> Result<Db> {
    let db = Db::open(app_db_open_options(storage, false)).await?;
    if database_uses_cloudsync_schema(&db).await? {
        db.pool().close().await;
        tracing::error!(
            %cloudsync_error,
            %probe_error,
            "cloudsync extension is unavailable for an initialized local replica"
        );
        return Err(cloudsync_error.into());
    }

    if let Err(error) = hypr_db_app::prepare_schema(&db).await {
        db.pool().close().await;
        return Err(error.into());
    }

    tracing::warn!(
        %cloudsync_error,
        %probe_error,
        "cloudsync extension is unavailable; opened the app database in local-only mode"
    );
    Ok(db)
}

async fn database_uses_cloudsync_schema(db: &Db) -> std::result::Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM sqlite_master
            WHERE (type = 'table' AND name = 'cloudsync_table_settings')
               OR (type = 'trigger' AND instr(lower(COALESCE(sql, '')), 'cloudsync_') > 0)
        )",
    )
    .fetch_one(db.pool())
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloudsync_completion_requires_confirmed_send_and_receive() {
        let completed: hypr_db_core::CloudsyncNetworkResult =
            serde_json::from_value(serde_json::json!({
                "send": {
                    "status": "synced",
                    "localVersion": 2,
                    "serverVersion": 2
                },
                "receive": {
                    "rows": 0,
                    "tables": [],
                    "complete": true
                }
            }))
            .unwrap();
        let unconfirmed_send: hypr_db_core::CloudsyncNetworkResult =
            serde_json::from_value(serde_json::json!({
                "send": {
                    "status": "syncing",
                    "localVersion": 2,
                    "serverVersion": 1
                }
            }))
            .unwrap();
        let incomplete_receive: hypr_db_core::CloudsyncNetworkResult =
            serde_json::from_value(serde_json::json!({
                "send": {
                    "status": "synced",
                    "localVersion": 2,
                    "serverVersion": 2
                },
                "receive": {
                    "rows": 1,
                    "tables": ["sessions"],
                    "complete": false
                }
            }))
            .unwrap();
        let send_only: hypr_db_core::CloudsyncNetworkResult =
            serde_json::from_value(serde_json::json!({
                "send": {
                    "status": "synced",
                    "localVersion": 2,
                    "serverVersion": 2
                }
            }))
            .unwrap();

        assert!(cloudsync_send_completed(&completed));
        assert!(cloudsync_snapshot_completed(&completed));
        assert!(!cloudsync_send_completed(&unconfirmed_send));
        assert!(!cloudsync_snapshot_completed(&unconfirmed_send));
        assert!(!cloudsync_send_completed(&incomplete_receive));
        assert!(!cloudsync_snapshot_completed(&incomplete_receive));
        assert!(cloudsync_send_completed(&send_only));
        assert!(!cloudsync_snapshot_completed(&send_only));
        assert!(!cloudsync_send_completed(
            &hypr_db_core::CloudsyncNetworkResult::default()
        ));
    }

    #[tokio::test]
    async fn reconciliation_barrier_blocks_renderer_writes() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        let runtime = std::sync::Arc::new(PluginDbRuntime::new(db));
        let guard = runtime.synced_write_barrier.write().await;

        let execute_runtime = std::sync::Arc::clone(&runtime);
        let mut execute = tokio::spawn(async move {
            execute_runtime
                .execute(
                    "INSERT INTO sessions (id, title) VALUES ('session-1', 'Session 1')"
                        .to_string(),
                    vec![],
                )
                .await
        });
        let transaction_runtime = std::sync::Arc::clone(&runtime);
        let mut transaction = tokio::spawn(async move {
            transaction_runtime
                .execute_transaction(vec![TransactionStatement {
                    sql: "INSERT INTO sessions (id, title) VALUES ('session-2', 'Session 2')"
                        .to_string(),
                    params: vec![],
                    expected_rows_affected: Some(1),
                }])
                .await
        });
        let proxy_runtime = std::sync::Arc::clone(&runtime);
        let mut proxy = tokio::spawn(async move {
            proxy_runtime
                .execute_proxy(
                    "INSERT INTO sessions (id, title) VALUES ('session-3', 'Session 3')"
                        .to_string(),
                    vec![],
                    ProxyQueryMethod::Run,
                )
                .await
        });

        let timeout = std::time::Duration::from_millis(25);
        assert!(tokio::time::timeout(timeout, &mut execute).await.is_err());
        assert!(
            tokio::time::timeout(timeout, &mut transaction)
                .await
                .is_err()
        );
        assert!(tokio::time::timeout(timeout, &mut proxy).await.is_err());
        drop(guard);

        execute.await.unwrap().unwrap();
        transaction.await.unwrap().unwrap();
        proxy.await.unwrap().unwrap();

        let session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(runtime.pool())
            .await
            .unwrap();
        assert_eq!(session_count, 3);
    }

    #[tokio::test]
    async fn reconciliation_barrier_blocks_native_synced_writes() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        let runtime = std::sync::Arc::new(PluginDbRuntime::new(db));
        let guard = runtime.synced_write_barrier.write().await;
        let write_runtime = std::sync::Arc::clone(&runtime);
        let mut write = tokio::spawn(async move {
            let _guard = write_runtime.synced_write_guard().await;
        });

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(25), &mut write)
                .await
                .is_err()
        );
        drop(guard);
        write.await.unwrap();
    }

    fn unavailable_extension_error() -> DbOpenError {
        DbOpenError::Io(std::io::Error::other("cloudsync extension unavailable"))
    }

    fn failed_extension_probe_error() -> DbOpenError {
        DbOpenError::Io(std::io::Error::other("cloudsync extension probe failed"))
    }

    #[tokio::test]
    async fn cloudsync_open_failure_falls_back_for_uninitialized_database() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");

        let db = open_app_db_without_cloudsync(
            DbStorage::Local(&db_path),
            unavailable_extension_error(),
            failed_extension_probe_error(),
        )
        .await
        .unwrap();

        assert!(!db.cloudsync_enabled());
        let sessions_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sessions'
            )",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert!(sessions_exists);
    }

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    #[tokio::test]
    async fn extension_open_without_initialized_tables_allows_plain_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = Db::open(DbOpenOptions {
            storage: DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        db.pool().close().await;
        drop(db);

        let db = open_app_db_without_cloudsync(
            DbStorage::Local(&db_path),
            unavailable_extension_error(),
            failed_extension_probe_error(),
        )
        .await
        .unwrap();

        assert!(!db.cloudsync_enabled());
        assert!(!database_uses_cloudsync_schema(&db).await.unwrap());
    }

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    #[tokio::test]
    async fn cloudsync_open_failure_does_not_migrate_initialized_replica_plainly() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = Db::open(DbOpenOptions {
            storage: DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.cloudsync_init("items", None, None).await.unwrap();
        db.pool().close().await;
        drop(db);

        let error = open_app_db_without_cloudsync(
            DbStorage::Local(&db_path),
            unavailable_extension_error(),
            failed_extension_probe_error(),
        )
        .await
        .unwrap_err();

        assert!(matches!(error, crate::Error::Db(DbOpenError::Io(_))));
        let plain = Db::connect_local_plain(&db_path).await.unwrap();
        let sessions_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sessions'
            )",
        )
        .fetch_one(plain.pool())
        .await
        .unwrap();
        assert!(!sessions_exists);
    }

    #[tokio::test]
    async fn cloudsync_open_fallback_propagates_schema_errors() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = Db::open(app_db_open_options(DbStorage::Local(&db_path), false))
            .await
            .unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "UPDATE app_settings
             SET value_json = 'not-json'
             WHERE id = 'cloudsync_workspace_binding'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.pool().close().await;
        drop(db);

        let error = open_app_db_without_cloudsync(
            DbStorage::Local(&db_path),
            unavailable_extension_error(),
            failed_extension_probe_error(),
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            crate::Error::AppSchema(hypr_db_app::AppSchemaError::CloudsyncWorkspace(
                hypr_db_app::CloudsyncWorkspaceError::InvalidBinding
            ))
        ));
    }

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

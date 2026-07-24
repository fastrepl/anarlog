use std::collections::HashMap;
use std::path::Path;

use hypr_db_core::{Db, DbOpenError, DbOpenOptions, DbStorage};
use hypr_db_execute::{DbExecutor, ProxyQueryMethod, ProxyQueryResult};
use hypr_db_reactive::{LiveQueryRuntime, QueryEventSink, SubscriptionRegistration};
use tauri::ipc::Channel;

use crate::{QueryEvent, Result, TransactionStatement};

const DEFAULT_CLOUDSYNC_INTERVAL_MS: u64 = 30_000;
const CLOUDSYNC_FULL_RESYNC_PROGRESS_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(200);
const CLOUDSYNC_FULL_RESYNC_RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
const CLOUDSYNC_RECOVERY_DELAYED_AFTER: std::time::Duration = std::time::Duration::from_secs(60);
const CLOUDSYNC_STATUS_ENRICHMENT_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(250);
const CLOUDSYNC_REPLICA_TABLE: &str = "e2ee_records";
const E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT: i64 = 64;
const E2EE_CLOUDSYNC_WITNESS_REPAIR_BYTE_LIMIT: usize = 16 * 1024 * 1024;
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
        hypr_db_app::encrypt_e2ee_replica_changes_deferring_active_captures(pool, &self.snapshot())
            .await
            .map(|_| ())
    }
}

impl hypr_db_core::CloudsyncSyncHook for E2eeSyncHook {
    fn before_sync<'a>(
        &'a self,
        pool: &'a sqlx::SqlitePool,
    ) -> hypr_db_core::CloudsyncBeforeHookFuture<'a> {
        let keys = self.snapshot();
        let witness = self.witness();
        Box::pin(async move {
            let witness = witness
                .ok_or_else(|| std::io::Error::other("E2EE freshness witness is not configured"))?;
            let key = keys.get(witness.workspace_id()).ok_or_else(|| {
                std::io::Error::other("E2EE freshness witness identity is not configured")
            })?;
            let full_resync_pending = hypr_db_app::cloudsync_full_resync_generation(pool)
                .await
                .map_err(|error| {
                    std::io::Error::other(format!(
                        "failed to inspect CloudSync full resync state: {error}"
                    ))
                })?
                .is_some();
            if full_resync_pending {
                tracing::debug!(
                    "skipping outbound E2EE publication while CloudSync hydrates a clean snapshot"
                );
                return Ok(hypr_db_core::CloudsyncSyncDirective::ReceiveOnly);
            }
            witness.refresh(pool, key).await?;
            let stats =
                hypr_db_app::encrypt_e2ee_replica_changes_bounded_deferring_active_captures(
                    pool,
                    &keys,
                    E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT,
                )
                .await
                .map_err(|error| {
                    std::io::Error::other(format!("E2EE pre-sync encryption failed: {error}"))
                })?;
            tracing::debug!(
                encrypted_fields = stats.encrypted_fields,
                "prepared encrypted CloudSync replica"
            );
            witness.publish_and_refresh(pool, key).await?;
            Ok(hypr_db_core::CloudsyncSyncDirective::SendAndReceive)
        })
    }

    fn after_sync<'a>(
        &'a self,
        pool: &'a sqlx::SqlitePool,
        result: &'a hypr_db_core::CloudsyncNetworkResult,
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
            let recovery_pending = hypr_db_app::cloudsync_full_resync_generation(pool)
                .await
                .map_err(|error| {
                    std::io::Error::other(format!(
                        "failed to inspect CloudSync full resync state: {error}"
                    ))
                })?
                .is_some();
            let snapshot_complete = !recovery_pending && cloudsync_receive_completed(result);
            let stats = hypr_db_app::apply_received_e2ee_replica_changes_with_witness(
                pool,
                &keys,
                snapshot_complete,
            )
            .await
            .map_err(|error| {
                std::io::Error::other(format!("E2EE post-sync decryption failed: {error}"))
            })?;
            tracing::debug!(
                applied_fields = stats.applied_fields,
                skipped_local_changes = stats.skipped_local_changes,
                rejected_rollbacks = stats.rejected_rollbacks,
                rejected_unwitnessed = stats.rejected_unwitnessed,
                snapshot_complete,
                "applied encrypted CloudSync replica"
            );
            let local_work_remaining = stats.remaining_replica_changes
                || hypr_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(
                    pool, &keys,
                )
                .await
                .map_err(|error| {
                    std::io::Error::other(format!(
                        "failed to inspect pending E2EE replica changes: {error}"
                    ))
                })?;
            Ok(hypr_db_core::CloudsyncHookOutcome {
                local_work_remaining,
            })
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
    scheduled_cloudsync_full_resync: std::sync::Arc<std::sync::Mutex<CloudsyncFullResyncSchedule>>,
    cloudsync_full_resync_task: tokio::sync::Mutex<Option<CloudsyncFullResyncTask>>,
}

struct CloudsyncFullResyncTask {
    #[cfg(test)]
    config: hypr_db_core::CloudsyncRuntimeConfig,
    #[cfg(test)]
    generation: String,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    join_handle: tokio::task::JoinHandle<()>,
}

#[derive(Default)]
struct CloudsyncFullResyncSchedule {
    generation: Option<String>,
    recovery_delayed: bool,
    last_progress_at: Option<std::time::Instant>,
}

#[derive(Clone, Copy)]
enum CloudsyncRecoveryStep {
    Progressed,
    Waiting,
    Complete,
}

fn cloudsync_recovery_step_delay(step: CloudsyncRecoveryStep) -> Option<std::time::Duration> {
    match step {
        CloudsyncRecoveryStep::Progressed => Some(CLOUDSYNC_FULL_RESYNC_PROGRESS_INTERVAL),
        CloudsyncRecoveryStep::Waiting => Some(CLOUDSYNC_FULL_RESYNC_RETRY_INTERVAL),
        CloudsyncRecoveryStep::Complete => None,
    }
}

#[derive(Clone, Copy)]
enum CloudsyncPendingFlush {
    Empty,
    Progressed,
    Waiting,
}

impl CloudsyncFullResyncSchedule {
    fn claim(&mut self, generation: &str) {
        self.generation = Some(generation.to_string());
        self.recovery_delayed = false;
        self.last_progress_at = Some(std::time::Instant::now());
    }

    fn is_active(&self, generation: &str) -> bool {
        self.generation.as_deref() == Some(generation)
    }

    fn cancel(&mut self) {
        self.generation = None;
        self.recovery_delayed = false;
        self.last_progress_at = None;
    }

    fn complete(&mut self, generation: &str) {
        if self.is_active(generation) {
            self.cancel();
        }
    }

    fn mark_progress(&mut self, generation: &str) {
        if self.is_active(generation) {
            self.recovery_delayed = false;
            self.last_progress_at = Some(std::time::Instant::now());
        }
    }

    fn mark_failure(&mut self, generation: &str) {
        if self.is_active(generation) {
            self.recovery_delayed = true;
        }
    }

    fn is_delayed(&self, generation: &str) -> bool {
        self.is_active(generation)
            && (self.recovery_delayed
                || self.last_progress_at.is_some_and(|last_progress_at| {
                    last_progress_at.elapsed() >= CLOUDSYNC_RECOVERY_DELAYED_AFTER
                }))
    }
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
            scheduled_cloudsync_full_resync: Default::default(),
            cloudsync_full_resync_task: Default::default(),
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
        crate::import::cleanup_legacy_files(self.db.pool()).await
    }

    pub async fn rerun_legacy_import(&self, dry_run: bool) -> Result<String> {
        let _write_guard = self.synced_write_barrier.read().await;
        crate::import::rerun_legacy_import(self.db.pool(), dry_run).await
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
        self.cancel_cloudsync_full_resync().await;
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
            self.cancel_cloudsync_full_resync().await;
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
        let witness =
            crate::e2ee_witness::E2eeWitnessClient::new(e2ee_witness, personal_workspace_id)?;
        let key = self
            .e2ee_sync_hook
            .workspace_key(personal_workspace_id)
            .ok_or(crate::Error::E2eeIdentityRequired)?;
        self.prepare_e2ee_cutover_and_initialize_witness(&witness, &key)
            .await?;
        self.e2ee_sync_hook.set_witness(witness);
        let config = hypr_db_core::CloudsyncRuntimeConfig {
            connection_string: database_id,
            auth: hypr_db_core::CloudsyncAuth::Token { token },
            tables: hypr_db_app::cloudsync_table_registry().to_vec(),
            sync_interval_ms: DEFAULT_CLOUDSYNC_INTERVAL_MS,
            wait_ms: Some(5_000),
            max_retries: Some(3),
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

        let reconciliation = match workspace_projection.as_ref() {
            Some(projection) => {
                let _write_guard = self.synced_write_barrier.write().await;
                Some(
                    hypr_db_app::stage_cloudsync_workspace_reconciliation(
                        self.db.pool(),
                        projection,
                    )
                    .await?,
                )
            }
            None => None,
        };
        if let Some(projection) = workspace_projection.as_ref() {
            hypr_db_app::set_cloudsync_personal_write_scope(
                self.db.pool(),
                &projection.personal_workspace_id,
            )
            .await?;
            let _write_guard = self.synced_write_barrier.write().await;
            let _ = hypr_db_app::commit_cloudsync_workspace_projection(
                self.db.pool(),
                projection,
                reconciliation
                    .as_ref()
                    .is_some_and(|plan| plan.requires_full_resync())
                    || !write_filter_installed,
            )
            .await?;
        }

        if let Some(generation) =
            hypr_db_app::cloudsync_full_resync_generation(self.db.pool()).await?
        {
            hypr_db_app::ensure_cloudsync_recovery_state(
                self.db.pool(),
                &generation,
                &account_user_id,
                personal_workspace_id,
                &key,
            )
            .await?;
            self.db.cloudsync_suspend().await?;
            self.db
                .cloudsync_prepare_manual_transport(config.clone())
                .await?;
            if hypr_db_app::cloudsync_recovery_state(self.db.pool())
                .await?
                .is_some_and(|state| {
                    state.generation == generation
                        && state.phase == hypr_db_app::CloudsyncRecoveryPhase::NeedFirstLogout
                })
            {
                discard_cloudsync_recovery_replica(self.db.as_ref(), &config, &generation).await?;
            }
            self.schedule_cloudsync_full_resync(generation, config)
                .await;
        } else {
            let (pending_fits, pending_chunks, pending_rows, pending_bytes) = self
                .prepare_cloudsync_config_fail_closed(config.clone())
                .await?;
            if pending_fits {
                self.db.cloudsync_resume_prepared_transport().await?;
            } else {
                tracing::warn!(
                    chunks = pending_chunks,
                    rows = pending_rows,
                    bytes = pending_bytes,
                    "recovering an oversized CloudSync outbox before it reaches the server",
                );
                let generation = prepare_cloudsync_poison_recovery(
                    self.db.as_ref(),
                    &account_user_id,
                    personal_workspace_id,
                    &key,
                )
                .await?;
                discard_cloudsync_recovery_replica(self.db.as_ref(), &config, &generation).await?;
                self.schedule_cloudsync_full_resync(generation, config)
                    .await;
            }
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

    async fn prepare_cloudsync_config_fail_closed(
        &self,
        config: hypr_db_core::CloudsyncRuntimeConfig,
    ) -> Result<(bool, u32, u64, u64)> {
        let result: Result<(bool, u32, u64, u64)> = async {
            self.db.cloudsync_stop().await?;
            self.db.cloudsync_prepare_manual_transport(config).await?;
            let batch = self.db.cloudsync_manual_pending_payload_batch().await?;
            Ok((batch.fits, batch.chunks, batch.rows, batch.bytes))
        }
        .await;

        match result {
            Ok(batch) => Ok(batch),
            Err(error) => {
                let _ = self.db.cloudsync_suspend().await;
                Err(error)
            }
        }
    }

    async fn prepare_e2ee_cutover(&self) -> Result<()> {
        if !self.legacy_e2ee_cutover_required().await? {
            return Ok(());
        }

        self.e2ee_sync_hook
            .prepare_local_snapshot(self.db.pool())
            .await
            .map_err(|error| std::io::Error::other(error.to_string()))?;

        let _write_guard = self.synced_write_barrier.write().await;
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

    async fn prepare_e2ee_cutover_and_initialize_witness(
        &self,
        witness: &crate::e2ee_witness::E2eeWitnessClient,
        key: &hypr_e2ee::WorkspaceKey,
    ) -> Result<()> {
        self.prepare_e2ee_cutover().await?;
        witness.initialize(self.db.pool(), key).await?;
        Ok(())
    }

    async fn legacy_e2ee_cutover_required(&self) -> Result<bool> {
        for table_name in hypr_db_app::E2EE_DOMAIN_TABLES {
            if hypr_db_core::cloudsync_is_enabled_on(self.db.pool(), table_name)
                .await
                .map_err(hypr_db_core::CloudsyncRuntimeError::from)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn schedule_cloudsync_full_resync(
        &self,
        generation: String,
        config: hypr_db_core::CloudsyncRuntimeConfig,
    ) {
        let mut task_slot = self.cloudsync_full_resync_task.lock().await;
        self.scheduled_cloudsync_full_resync
            .lock()
            .unwrap()
            .cancel();
        if let Some(task) = task_slot.take() {
            join_cloudsync_full_resync_task(task).await;
        }
        self.scheduled_cloudsync_full_resync
            .lock()
            .unwrap()
            .claim(&generation);

        #[cfg(test)]
        let task_config = config.clone();
        #[cfg(test)]
        let task_generation = generation.clone();
        let db = std::sync::Arc::clone(&self.db);
        let scheduled = std::sync::Arc::clone(&self.scheduled_cloudsync_full_resync);
        let e2ee_sync_hook = std::sync::Arc::clone(&self.e2ee_sync_hook);
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        let join_handle = tokio::spawn(async move {
            let mut clean_receive_attempt_started = false;
            loop {
                if !scheduled.lock().unwrap().is_active(&generation) {
                    return;
                }

                let step: Result<CloudsyncRecoveryStep> = tokio::select! {
                    biased;
                    _ = &mut shutdown_rx => return,
                    step = async {
                        let state = hypr_db_app::cloudsync_recovery_state(db.pool())
                            .await?
                            .ok_or_else(|| {
                                std::io::Error::other(
                                    "CloudSync recovery state disappeared while resync is pending",
                                )
                            })?;
                    if state.generation != generation {
                        return Err(std::io::Error::other(
                            "CloudSync recovery generation changed while running",
                        )
                        .into());
                    }
                    let key = e2ee_sync_hook
                        .workspace_key(&state.workspace_id)
                        .ok_or(crate::Error::E2eeIdentityRequired)?;
                    let witness = e2ee_sync_hook.witness().ok_or_else(|| {
                        std::io::Error::other("E2EE freshness witness is not configured")
                    })?;
                    if witness.workspace_id() != state.workspace_id {
                        return Err(std::io::Error::other(
                            "E2EE freshness witness workspace changed during recovery",
                        )
                        .into());
                    }

                    match state.phase {
                        hypr_db_app::CloudsyncRecoveryPhase::NeedFirstLogout => {
                            discard_cloudsync_recovery_replica(
                                db.as_ref(),
                                &config,
                                &generation,
                            )
                            .await?;
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert => {
                            hypr_db_app::insert_cloudsync_recovery_barrier(
                                db.pool(),
                                &state,
                                &key,
                            )
                            .await?;
                            if !hypr_db_app::advance_cloudsync_recovery_phase(
                                db.pool(),
                                &generation,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm,
                            )
                            .await?
                            {
                                return Err(hypr_db_app::CloudsyncWorkspaceError::RecoveryConflict
                                    .into());
                            }
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm => {
                            match flush_manual_cloudsync_pending(db.as_ref()).await? {
                                CloudsyncPendingFlush::Progressed => {
                                    return Ok(CloudsyncRecoveryStep::Progressed);
                                }
                                CloudsyncPendingFlush::Waiting => {
                                    return Ok(CloudsyncRecoveryStep::Waiting);
                                }
                                CloudsyncPendingFlush::Empty => {}
                            }
                            if !hypr_db_app::cloudsync_recovery_barrier_is_exact(
                                db.pool(),
                                &state,
                                &key,
                            )
                            .await?
                            {
                                return Err(std::io::Error::other(
                                    "CloudSync recovery barrier disappeared before send confirmation",
                                )
                                .into());
                            }
                            if !hypr_db_app::advance_cloudsync_recovery_phase(
                                db.pool(),
                                &generation,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedCleanReceive,
                            )
                            .await?
                            {
                                return Err(hypr_db_app::CloudsyncWorkspaceError::RecoveryConflict
                                    .into());
                            }
                            clean_receive_attempt_started = false;
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        hypr_db_app::CloudsyncRecoveryPhase::NeedCleanReceive => {
                            if !clean_receive_attempt_started {
                                require_disposable_cloudsync_replica(db.as_ref()).await?;
                                db.cloudsync_logout(true).await?;
                                prepare_manual_cloudsync_recovery_transport(
                                    db.as_ref(),
                                    &config,
                                )
                                .await?;
                                db.cloudsync_network_reset_receive_version()
                                    .await
                                    .map_err(hypr_db_core::CloudsyncRuntimeError::from)?;
                                if !hypr_db_app::cloudsync_encrypted_replica_is_empty(db.pool())
                                    .await?
                                {
                                    return Err(std::io::Error::other(
                                        "CloudSync replica was not empty before clean recovery receive",
                                    )
                                    .into());
                                }
                                clean_receive_attempt_started = true;
                            }

                            let result = db.cloudsync_manual_receive_one().await?;
                            witness.refresh(db.pool(), &key).await?;
                            let apply =
                                hypr_db_app::apply_received_e2ee_replica_changes_with_witness(
                                db.pool(),
                                &e2ee_sync_hook.snapshot(),
                                false,
                            )
                            .await
                            .map_err(|error| std::io::Error::other(error.to_string()))?;
                            let barrier_is_exact =
                                hypr_db_app::cloudsync_recovery_barrier_is_exact(
                                    db.pool(),
                                    &state,
                                    &key,
                                )
                                .await?;
                            if cloudsync_recovery_snapshot_ready(barrier_is_exact, &result) {
                                if !hypr_db_app::advance_cloudsync_recovery_phase(
                                    db.pool(),
                                    &generation,
                                    hypr_db_app::CloudsyncRecoveryPhase::NeedCleanReceive,
                                    hypr_db_app::CloudsyncRecoveryPhase::NeedWitnessRepair,
                                )
                                .await?
                                {
                                    return Err(
                                        hypr_db_app::CloudsyncWorkspaceError::RecoveryConflict
                                            .into(),
                                    );
                                }
                                clean_receive_attempt_started = false;
                                return Ok(CloudsyncRecoveryStep::Progressed);
                            }
                            if cloudsync_receive_delivered(&result) || apply.applied_fields > 0 {
                                return Ok(CloudsyncRecoveryStep::Progressed);
                            }
                            if apply.remaining_replica_changes {
                                return Ok(CloudsyncRecoveryStep::Waiting);
                            }
                            Ok(CloudsyncRecoveryStep::Waiting)
                        }
                        hypr_db_app::CloudsyncRecoveryPhase::NeedWitnessRepair => {
                            match flush_manual_cloudsync_pending(db.as_ref()).await? {
                                CloudsyncPendingFlush::Progressed => {
                                    return Ok(CloudsyncRecoveryStep::Progressed);
                                }
                                CloudsyncPendingFlush::Waiting => {
                                    return Ok(CloudsyncRecoveryStep::Waiting);
                                }
                                CloudsyncPendingFlush::Empty => {}
                            }

                            witness.refresh(db.pool(), &key).await?;
                            witness.publish_and_refresh(db.pool(), &key).await?;
                            let keys = e2ee_sync_hook.snapshot();
                            let repair =
                                hypr_db_app::repair_e2ee_replica_from_witness_bounded(
                                    db.pool(),
                                    &keys,
                                    true,
                                    E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT,
                                    E2EE_CLOUDSYNC_WITNESS_REPAIR_BYTE_LIMIT,
                                )
                                .await
                                .map_err(|error| std::io::Error::other(error.to_string()))?;
                            let apply =
                                hypr_db_app::apply_received_e2ee_replica_changes_with_witness(
                                db.pool(),
                                &keys,
                                false,
                            )
                            .await
                            .map_err(|error| std::io::Error::other(error.to_string()))?;
                            if repair.repaired_records > 0 || apply.applied_fields > 0 {
                                return Ok(CloudsyncRecoveryStep::Progressed);
                            }
                            if repair.remaining || apply.remaining_replica_changes {
                                return Ok(CloudsyncRecoveryStep::Waiting);
                            }

                            witness.refresh(db.pool(), &key).await?;
                            if hypr_db_app::has_pending_e2ee_witness_repairs(
                                db.pool(),
                                &keys,
                                true,
                            )
                            .await
                            .map_err(|error| std::io::Error::other(error.to_string()))?
                            {
                                return Ok(CloudsyncRecoveryStep::Waiting);
                            }

                            let local = hypr_db_app::encrypt_e2ee_replica_changes_bounded_deferring_active_captures(
                                    db.pool(),
                                    &keys,
                                    E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT,
                                )
                                .await
                                .map_err(|error| std::io::Error::other(error.to_string()))?;
                            if local.encrypted_fields > 0 {
                                witness.publish_and_refresh(db.pool(), &key).await?;
                                return Ok(CloudsyncRecoveryStep::Progressed);
                            }
                            if hypr_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(
                                db.pool(), &keys,
                            )
                            .await
                            .map_err(|error| std::io::Error::other(error.to_string()))?
                            {
                                return Ok(CloudsyncRecoveryStep::Waiting);
                            }

                            witness.refresh(db.pool(), &key).await?;
                            if hypr_db_app::has_pending_e2ee_witness_repairs(
                                db.pool(),
                                &keys,
                                true,
                            )
                            .await
                            .map_err(|error| std::io::Error::other(error.to_string()))?
                            {
                                return Ok(CloudsyncRecoveryStep::Waiting);
                            }
                            if !matches!(
                                flush_manual_cloudsync_pending(db.as_ref()).await?,
                                CloudsyncPendingFlush::Empty
                            ) {
                                return Ok(CloudsyncRecoveryStep::Waiting);
                            }
                            if !hypr_db_app::advance_cloudsync_recovery_phase(
                                db.pool(),
                                &generation,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedWitnessRepair,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierCleanup,
                            )
                            .await?
                            {
                                return Err(hypr_db_app::CloudsyncWorkspaceError::RecoveryConflict
                                    .into());
                            }
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierCleanup => {
                            match flush_manual_cloudsync_pending(db.as_ref()).await? {
                                CloudsyncPendingFlush::Progressed => {
                                    return Ok(CloudsyncRecoveryStep::Progressed);
                                }
                                CloudsyncPendingFlush::Waiting => {
                                    return Ok(CloudsyncRecoveryStep::Waiting);
                                }
                                CloudsyncPendingFlush::Empty => {}
                            }
                            if hypr_db_app::cloudsync_recovery_barrier_is_exact(
                                db.pool(),
                                &state,
                                &key,
                            )
                            .await?
                            {
                                hypr_db_app::delete_cloudsync_recovery_barrier(
                                    db.pool(),
                                    &state,
                                    &key,
                                )
                                .await?;
                                return Ok(CloudsyncRecoveryStep::Progressed);
                            }
                            if !hypr_db_app::advance_cloudsync_recovery_phase(
                                db.pool(),
                                &generation,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierCleanup,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedTransportResume,
                            )
                            .await?
                            {
                                return Err(hypr_db_app::CloudsyncWorkspaceError::RecoveryConflict
                                    .into());
                            }
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        hypr_db_app::CloudsyncRecoveryPhase::NeedTransportResume => {
                            db.cloudsync_resume_prepared_transport().await?;
                            if !hypr_db_app::complete_cloudsync_recovery(
                                db.pool(),
                                &generation,
                            )
                            .await?
                            {
                                return Err(hypr_db_app::CloudsyncWorkspaceError::RecoveryConflict
                                    .into());
                            }
                            Ok(CloudsyncRecoveryStep::Complete)
                        }
                    }
                    } => step,
                };

                let retry_delay = match step {
                    Ok(step) => {
                        match step {
                            CloudsyncRecoveryStep::Complete => {
                                scheduled.lock().unwrap().complete(&generation);
                                return;
                            }
                            CloudsyncRecoveryStep::Progressed => {
                                scheduled.lock().unwrap().mark_progress(&generation);
                            }
                            CloudsyncRecoveryStep::Waiting => {}
                        }
                        cloudsync_recovery_step_delay(step)
                            .expect("completed CloudSync recovery already returned")
                    }
                    Err(error) => {
                        scheduled.lock().unwrap().mark_failure(&generation);
                        tracing::warn!(%error, "CloudSync recovery remains pending");
                        CLOUDSYNC_FULL_RESYNC_RETRY_INTERVAL
                    }
                };
                tokio::select! {
                    biased;
                    _ = &mut shutdown_rx => return,
                    _ = tokio::time::sleep(retry_delay) => {}
                }
            }
        });
        *task_slot = Some(CloudsyncFullResyncTask {
            #[cfg(test)]
            config: task_config,
            #[cfg(test)]
            generation: task_generation,
            shutdown_tx: Some(shutdown_tx),
            join_handle,
        });
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

    async fn cancel_cloudsync_full_resync(&self) {
        let mut task_slot = self.cloudsync_full_resync_task.lock().await;
        self.scheduled_cloudsync_full_resync
            .lock()
            .unwrap()
            .cancel();
        if let Some(task) = task_slot.take() {
            join_cloudsync_full_resync_task(task).await;
        }
    }

    #[cfg(test)]
    pub(crate) async fn cloudsync_full_resync_task_snapshot(
        &self,
    ) -> Option<(String, hypr_db_core::CloudsyncAuth)> {
        self.cloudsync_full_resync_task
            .lock()
            .await
            .as_ref()
            .map(|task| (task.generation.clone(), task.config.auth.clone()))
    }

    pub async fn start_cloudsync(&self) -> Result<()> {
        self.ensure_legacy_migration_verified().await?;
        self.db.cloudsync_start().await?;
        Ok(())
    }

    pub async fn stop_cloudsync(&self) -> Result<()> {
        self.cancel_cloudsync_full_resync().await;
        self.db.cloudsync_stop().await?;
        Ok(())
    }

    pub async fn suspend_cloudsync(&self) -> Result<()> {
        self.cancel_cloudsync_full_resync().await;
        self.db.cloudsync_suspend().await?;
        self.e2ee_sync_hook.clear();
        Ok(())
    }

    pub async fn cloudsync_status(&self) -> Result<serde_json::Value> {
        // Read the canonical dirty queue before the native outbox so a concurrent
        // promotion is visible in at least one status snapshot.
        let keys = self.e2ee_sync_hook.snapshot();
        let (local_e2ee_work_pending, recovery) =
            tokio::time::timeout(CLOUDSYNC_STATUS_ENRICHMENT_TIMEOUT, async {
                let local_e2ee_work_pending =
                    hypr_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(
                        self.db.pool(),
                        &keys,
                    )
                    .await
                    .map_err(|error| {
                        std::io::Error::other(format!(
                            "failed to inspect pending E2EE replica changes: {error}"
                        ))
                    })?;
                let recovery = hypr_db_app::cloudsync_recovery_state(self.db.pool()).await?;
                Ok::<_, crate::Error>((local_e2ee_work_pending, recovery))
            })
            .await
            .map_err(|_| hypr_db_core::CloudsyncRuntimeError::LocalStatusBusy)??;
        let mut status = serde_json::to_value(self.db.cloudsync_status().await?)?;
        let recovery_delayed = recovery.as_ref().is_some_and(|state| {
            self.scheduled_cloudsync_full_resync
                .lock()
                .unwrap()
                .is_delayed(&state.generation)
        });
        let status_object = status.as_object_mut().ok_or_else(|| {
            std::io::Error::other("CloudSync status did not serialize to an object")
        })?;
        if local_e2ee_work_pending {
            status_object.insert(
                "has_unsent_changes".to_string(),
                serde_json::Value::Bool(true),
            );
        }
        status_object.insert(
            "recovery_pending".to_string(),
            serde_json::Value::Bool(recovery.is_some()),
        );
        status_object.insert(
            "recovery_delayed".to_string(),
            serde_json::Value::Bool(recovery_delayed),
        );
        status_object.insert(
            "recovery_phase".to_string(),
            recovery
                .map(|state| serde_json::to_value(state.phase))
                .transpose()?
                .unwrap_or(serde_json::Value::Null),
        );
        Ok(status)
    }

    pub async fn sync_cloudsync_now(&self) -> Result<serde_json::Value> {
        self.ensure_legacy_migration_verified().await?;
        Ok(serde_json::to_value(
            self.db.cloudsync_trigger_sync().await?,
        )?)
    }

    pub async fn logout_cloudsync(&self, discard_unsent_changes: bool) -> Result<()> {
        self.cancel_cloudsync_full_resync().await;
        let _write_guard = self.synced_write_barrier.write().await;
        self.db.cloudsync_logout(discard_unsent_changes).await?;
        self.e2ee_sync_hook.clear();
        Ok(())
    }
}

impl Drop for PluginDbRuntime {
    fn drop(&mut self) {
        self.scheduled_cloudsync_full_resync
            .lock()
            .unwrap()
            .cancel();
        if let Some(mut task) = self.cloudsync_full_resync_task.get_mut().take() {
            if let Some(shutdown_tx) = task.shutdown_tx.take() {
                let _ = shutdown_tx.send(());
            }
            task.join_handle.abort();
        }
    }
}

async fn join_cloudsync_full_resync_task(mut task: CloudsyncFullResyncTask) {
    if let Some(shutdown_tx) = task.shutdown_tx.take() {
        let _ = shutdown_tx.send(());
    }
    if let Err(error) = task.join_handle.await
        && !error.is_cancelled()
    {
        tracing::warn!(%error, "CloudSync recovery task failed while stopping");
    }
}

async fn require_disposable_cloudsync_replica(db: &Db) -> Result<()> {
    let settings_exist: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1
           FROM sqlite_master
           WHERE type = 'table' AND name = 'cloudsync_table_settings'
         )",
    )
    .fetch_one(db.pool())
    .await?;
    if !settings_exist {
        return Err(std::io::Error::other(
            "CloudSync recovery requires an initialized encrypted replica",
        )
        .into());
    }

    let tracked_tables: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT tbl_name
         FROM cloudsync_table_settings
         ORDER BY tbl_name",
    )
    .fetch_all(db.pool())
    .await?;
    if tracked_tables.as_slice() != [CLOUDSYNC_REPLICA_TABLE] {
        return Err(std::io::Error::other(format!(
            "CloudSync recovery can only discard {CLOUDSYNC_REPLICA_TABLE}; tracked tables: {}",
            tracked_tables.join(", ")
        ))
        .into());
    }
    Ok(())
}

async fn prepare_manual_cloudsync_recovery_transport(
    db: &Db,
    config: &hypr_db_core::CloudsyncRuntimeConfig,
) -> Result<()> {
    db.cloudsync_prepare_manual_transport(config.clone())
        .await?;
    if !hypr_db_app::cloudsync_encrypted_replica_is_empty(db.pool()).await? {
        return Err(std::io::Error::other(
            "CloudSync recovery refused to reinstall a filter on a populated replica",
        )
        .into());
    }
    db.cloudsync_set_filter(CLOUDSYNC_REPLICA_TABLE, CLOUDSYNC_WRITE_FILTER)
        .await
        .map_err(hypr_db_core::CloudsyncRuntimeError::from)?;
    hypr_db_app::mark_cloudsync_write_filter_installed(db.pool()).await?;
    require_disposable_cloudsync_replica(db).await
}

async fn prepare_cloudsync_poison_recovery(
    db: &Db,
    account_user_id: &str,
    workspace_id: &str,
    key: &hypr_e2ee::WorkspaceKey,
) -> Result<String> {
    require_disposable_cloudsync_replica(db).await?;
    let generation =
        hypr_db_app::stage_cloudsync_poison_recovery(db.pool(), account_user_id, workspace_id)
            .await?;
    hypr_db_app::ensure_cloudsync_recovery_state(
        db.pool(),
        &generation,
        account_user_id,
        workspace_id,
        key,
    )
    .await?;
    Ok(generation)
}

async fn discard_cloudsync_recovery_replica(
    db: &Db,
    config: &hypr_db_core::CloudsyncRuntimeConfig,
    generation: &str,
) -> Result<()> {
    require_disposable_cloudsync_replica(db).await?;
    db.cloudsync_logout(true).await?;
    prepare_manual_cloudsync_recovery_transport(db, config).await?;
    if !hypr_db_app::cloudsync_encrypted_replica_is_empty(db.pool()).await? {
        return Err(std::io::Error::other(
            "CloudSync replica was not empty after the first recovery logout",
        )
        .into());
    }
    if !hypr_db_app::advance_cloudsync_recovery_phase(
        db.pool(),
        generation,
        hypr_db_app::CloudsyncRecoveryPhase::NeedFirstLogout,
        hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert,
    )
    .await?
    {
        return Err(hypr_db_app::CloudsyncWorkspaceError::RecoveryConflict.into());
    }
    Ok(())
}

async fn flush_manual_cloudsync_pending(db: &Db) -> Result<CloudsyncPendingFlush> {
    let batch = db.cloudsync_manual_pending_payload_batch().await?;
    if !batch.complete || !batch.fits {
        return Err(std::io::Error::other(format!(
            "CloudSync pending payload is not safely bounded ({} chunks, {} bytes)",
            batch.chunks, batch.bytes
        ))
        .into());
    }
    if batch.chunks == 0 {
        let status = db.cloudsync_manual_network_status().await?;
        return Ok(
            if status.gaps.is_empty()
                && status.failures.apply.is_none()
                && status.last_optimistic_version >= batch.start_db_version
                && status.last_confirmed_version >= batch.start_db_version
            {
                CloudsyncPendingFlush::Empty
            } else {
                CloudsyncPendingFlush::Waiting
            },
        );
    }

    if let Ok(status) = db.cloudsync_manual_network_status().await
        && db
            .cloudsync_manual_reconcile_confirmed_pending_payload(batch, &status)
            .await?
    {
        return Ok(CloudsyncPendingFlush::Progressed);
    }

    match db.cloudsync_manual_send_only().await {
        Ok(result) if cloudsync_send_completed(&result) => Ok(CloudsyncPendingFlush::Progressed),
        Ok(result) if cloudsync_send_made_progress(&result) => {
            Ok(CloudsyncPendingFlush::Progressed)
        }
        Ok(_) => Ok(CloudsyncPendingFlush::Waiting),
        Err(send_error) => {
            let status = db.cloudsync_manual_network_status().await?;
            if db
                .cloudsync_manual_reconcile_confirmed_pending_payload(batch, &status)
                .await?
            {
                Ok(CloudsyncPendingFlush::Progressed)
            } else {
                Err(send_error.into())
            }
        }
    }
}

fn cloudsync_send_completed(result: &hypr_db_core::CloudsyncNetworkResult) -> bool {
    let Some(send) = result.send.as_ref() else {
        return false;
    };
    send.status.eq_ignore_ascii_case("synced") && send.last_failure.is_none()
}

fn cloudsync_send_made_progress(result: &hypr_db_core::CloudsyncNetworkResult) -> bool {
    result
        .send
        .as_ref()
        .is_some_and(|send| send.chunks > 0 && send.last_failure.is_none())
}

fn cloudsync_receive_completed(result: &hypr_db_core::CloudsyncNetworkResult) -> bool {
    result.receive.as_ref().is_some_and(|receive| {
        receive.complete && receive.error.is_none() && receive.last_failure.is_none()
    })
}

fn cloudsync_receive_delivered(result: &hypr_db_core::CloudsyncNetworkResult) -> bool {
    result.receive.as_ref().is_some_and(|receive| {
        receive.chunks > 0 && receive.error.is_none() && receive.last_failure.is_none()
    })
}

fn cloudsync_receive_delivered_final(result: &hypr_db_core::CloudsyncNetworkResult) -> bool {
    cloudsync_receive_delivered(result)
        && result
            .receive
            .as_ref()
            .is_some_and(|receive| receive.complete)
}

fn cloudsync_recovery_snapshot_ready(
    barrier_is_exact: bool,
    result: &hypr_db_core::CloudsyncNetworkResult,
) -> bool {
    barrier_is_exact && cloudsync_receive_delivered_final(result)
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
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate, matchers::path};

    #[derive(Clone, Default)]
    struct InitiallyUninitializedWitness {
        initialized: std::sync::Arc<AtomicBool>,
    }

    impl Respond for InitiallyUninitializedWitness {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            if request.method == wiremock::http::Method::POST {
                let body: serde_json::Value = request.body_json().unwrap();
                if !self.initialized.load(Ordering::SeqCst)
                    && body["initialize"].as_bool() != Some(true)
                {
                    return ResponseTemplate::new(409);
                }
                if body["events"].as_array().is_none_or(Vec::is_empty) {
                    return ResponseTemplate::new(400);
                }
                self.initialized.store(true, Ordering::SeqCst);
                return ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "initializedAt": "2026-07-23T00:00:00Z",
                    "headSequence": 0,
                }));
            }

            let initialized = self.initialized.load(Ordering::SeqCst);
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "initialized": initialized,
                "initializedAt": initialized.then_some("2026-07-23T00:00:00Z"),
                "headSequence": 0,
                "throughSequence": 0,
                "nextAfterSequence": 0,
                "events": [],
            }))
        }
    }

    #[tokio::test]
    async fn capture_lifecycle_marker_defers_only_its_transcript() {
        let db = Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let hook = E2eeSyncHook::default();
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        hook.set_personal_workspace("workspace-1", &recovery_key)
            .unwrap();
        let witness_state = InitiallyUninitializedWitness::default();
        witness_state.initialized.store(true, Ordering::SeqCst);
        let witness_server = MockServer::start().await;
        Mock::given(path("/sync/e2ee/witness/workspace-1"))
            .respond_with(witness_state)
            .mount(&witness_server)
            .await;
        hook.set_witness(
            crate::e2ee_witness::E2eeWitnessClient::new(
                crate::CloudsyncE2eeWitness {
                    endpoint: format!("{}/sync/e2ee/witness/workspace-1", witness_server.uri()),
                    access_token: "access-token".to_string(),
                },
                "workspace-1",
            )
            .unwrap(),
        );

        sqlx::query(
            "INSERT INTO app_settings (id, value_json)
             VALUES ('capture_lifecycle_pending:session-1', ?)",
        )
        .bind(
            serde_json::json!({
                "version": 1,
                "phase": "capturing",
                "sessionId": "session-1",
                "transcriptId": "transcript-1",
                "startedAt": 1_000,
                "createdAt": "2026-07-24T00:00:00.000Z",
                "audioOffsetMs": 0,
                "preserveExistingTranscript": false,
                "ownerUserId": "workspace-1",
                "memo": ""
            })
            .to_string(),
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, title)
             VALUES
               ('session-1', 'workspace-1', 'Active capture'),
               ('session-2', 'workspace-1', 'Unrelated edit')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcripts (id, workspace_id, session_id, words_json)
             VALUES
               ('transcript-1', 'workspace-1', 'session-1', '[{\"text\":\"partial\"}]'),
               ('transcript-2', 'workspace-1', 'session-2', '[{\"text\":\"complete\"}]')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        assert_eq!(
            hypr_db_core::CloudsyncSyncHook::before_sync(&hook, db.pool())
                .await
                .unwrap(),
            hypr_db_core::CloudsyncSyncDirective::SendAndReceive
        );
        assert!(!witness_server.received_requests().await.unwrap().is_empty());
        let remaining_dirty: Vec<(String, String)> = sqlx::query_as(
            "SELECT table_name, row_id
             FROM e2ee_dirty_rows
             ORDER BY table_name, row_id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(
            remaining_dirty,
            vec![("transcripts".to_string(), "transcript-1".to_string())]
        );
        let protected_records: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM e2ee_local_state
             WHERE table_name = 'transcripts' AND row_id = 'transcript-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(protected_records, 0);
        let unrelated_records: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM e2ee_local_state
             WHERE table_name = 'transcripts' AND row_id = 'transcript-2'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert!(unrelated_records > 0);

        let result: hypr_db_core::CloudsyncNetworkResult =
            serde_json::from_value(serde_json::json!({
                "receive": {
                    "rows": 0,
                    "tables": [],
                    "chunks": 0,
                    "bytes": 0,
                    "complete": true
                }
            }))
            .unwrap();
        let outcome = hypr_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &result)
            .await
            .unwrap();
        assert!(!outcome.local_work_remaining);
        let keys = hook.snapshot();

        sqlx::query(
            "UPDATE app_settings
             SET value_json = json_set(value_json, '$.phase', 'finalizing')
             WHERE id = 'capture_lifecycle_pending:session-1'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        assert!(
            hypr_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(
                db.pool(),
                &keys,
            )
            .await
            .unwrap()
        );

        sqlx::query(
            "INSERT INTO app_settings (id, value_json)
             VALUES
               ('capture_lifecycle_pending:malformed', '{}'),
               ('capture_lifecycle_pending:invalid-json', '{')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        assert!(
            hypr_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(
                db.pool(),
                &keys,
            )
            .await
            .unwrap()
        );

        sqlx::query(
            "UPDATE app_settings
             SET value_json = json_remove(value_json, '$.phase')
             WHERE id = 'capture_lifecycle_pending:session-1'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        assert!(
            !hypr_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(
                db.pool(),
                &keys,
            )
            .await
            .unwrap()
        );

        sqlx::query(
            "UPDATE app_settings
             SET value_json = json_set(value_json, '$.phase', 'unknown')
             WHERE id = 'capture_lifecycle_pending:session-1'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        assert!(
            !hypr_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(
                db.pool(),
                &keys,
            )
            .await
            .unwrap()
        );

        sqlx::query(
            "UPDATE app_settings
             SET value_json = json_set(value_json, '$.summaryMode', 'if_empty')
             WHERE id = 'capture_lifecycle_pending:session-1'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        assert!(
            hypr_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(
                db.pool(),
                &keys,
            )
            .await
            .unwrap()
        );

        sqlx::query(
            "UPDATE app_settings
             SET value_json = json_remove(
               json_set(value_json, '$.version', json('true')),
               '$.summaryMode'
             )
             WHERE id = 'capture_lifecycle_pending:session-1'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        assert!(
            hypr_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(
                db.pool(),
                &keys,
            )
            .await
            .unwrap()
        );

        sqlx::query(
            "UPDATE app_settings
             SET value_json = json_set(
               value_json,
               '$.version',
               1,
               '$.startedAt',
               1e999
             )
             WHERE id = 'capture_lifecycle_pending:session-1'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        assert!(
            hypr_db_app::has_pending_e2ee_replica_changes_deferring_active_captures(
                db.pool(),
                &keys,
            )
            .await
            .unwrap()
        );

        sqlx::query(
            "UPDATE app_settings
             SET value_json = json_set(
               value_json,
               '$.startedAt',
               1000,
               '$.summaryMode',
               'if_empty'
             )
             WHERE id = 'capture_lifecycle_pending:session-1'",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let outcome = hypr_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &result)
            .await
            .unwrap();
        assert!(outcome.local_work_remaining);

        assert_eq!(
            hypr_db_core::CloudsyncSyncHook::before_sync(&hook, db.pool())
                .await
                .unwrap(),
            hypr_db_core::CloudsyncSyncDirective::SendAndReceive
        );
        let protected_dirty: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM e2ee_dirty_rows
             WHERE table_name = 'transcripts' AND row_id = 'transcript-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(protected_dirty, 0);
    }

    #[test]
    fn recovery_waits_longer_without_progress() {
        assert_eq!(
            cloudsync_recovery_step_delay(CloudsyncRecoveryStep::Progressed),
            Some(CLOUDSYNC_FULL_RESYNC_PROGRESS_INTERVAL)
        );
        assert_eq!(
            cloudsync_recovery_step_delay(CloudsyncRecoveryStep::Waiting),
            Some(CLOUDSYNC_FULL_RESYNC_RETRY_INTERVAL)
        );
        assert_eq!(
            cloudsync_recovery_step_delay(CloudsyncRecoveryStep::Complete),
            None
        );
    }

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
        let receive_only: hypr_db_core::CloudsyncNetworkResult =
            serde_json::from_value(serde_json::json!({
                "receive": {
                    "rows": 0,
                    "tables": [],
                    "complete": true
                }
            }))
            .unwrap();
        let delivered_final: hypr_db_core::CloudsyncNetworkResult =
            serde_json::from_value(serde_json::json!({
                "receive": {
                    "rows": 1,
                    "tables": ["e2ee_records"],
                    "chunks": 1,
                    "complete": true
                }
            }))
            .unwrap();
        let delivered_non_final: hypr_db_core::CloudsyncNetworkResult =
            serde_json::from_value(serde_json::json!({
                "receive": {
                    "rows": 1,
                    "tables": ["e2ee_records"],
                    "chunks": 1,
                    "complete": false
                }
            }))
            .unwrap();
        let failed_receive: hypr_db_core::CloudsyncNetworkResult =
            serde_json::from_value(serde_json::json!({
                "send": {
                    "status": "synced",
                    "localVersion": 2,
                    "serverVersion": 2
                },
                "receive": {
                    "rows": 0,
                    "tables": [],
                    "complete": true,
                    "error": "database is locked"
                }
            }))
            .unwrap();

        assert!(cloudsync_send_completed(&completed));
        assert!(cloudsync_receive_completed(&completed));
        assert!(!cloudsync_send_completed(&unconfirmed_send));
        assert!(!cloudsync_receive_completed(&incomplete_receive));
        assert!(cloudsync_send_completed(&incomplete_receive));
        assert!(cloudsync_send_completed(&send_only));
        assert!(!cloudsync_receive_completed(&send_only));
        assert!(!cloudsync_send_completed(&receive_only));
        assert!(cloudsync_receive_completed(&receive_only));
        assert!(!cloudsync_receive_delivered_final(&receive_only));
        assert!(cloudsync_receive_delivered_final(&delivered_final));
        assert!(cloudsync_receive_delivered(&delivered_non_final));
        assert!(!cloudsync_recovery_snapshot_ready(
            true,
            &delivered_non_final
        ));
        assert!(cloudsync_recovery_snapshot_ready(true, &delivered_final));
        assert!(!cloudsync_recovery_snapshot_ready(true, &receive_only));
        assert!(!cloudsync_recovery_snapshot_ready(false, &delivered_final));
        assert!(cloudsync_send_completed(&failed_receive));
        assert!(!cloudsync_receive_completed(&failed_receive));
        assert!(!cloudsync_send_completed(
            &hypr_db_core::CloudsyncNetworkResult::default()
        ));
    }

    fn test_full_resync_config(token: &str) -> hypr_db_core::CloudsyncRuntimeConfig {
        hypr_db_core::CloudsyncRuntimeConfig {
            connection_string: "test-database".to_string(),
            auth: hypr_db_core::CloudsyncAuth::Token {
                token: token.to_string(),
            },
            tables: vec![],
            sync_interval_ms: DEFAULT_CLOUDSYNC_INTERVAL_MS,
            wait_ms: Some(5_000),
            max_retries: Some(3),
        }
    }

    #[test]
    fn full_resync_schedule_tracks_generation_until_cancelled() {
        let mut schedule = CloudsyncFullResyncSchedule::default();

        schedule.claim("generation-1");
        assert!(!schedule.is_delayed("generation-1"));
        schedule.last_progress_at =
            Some(std::time::Instant::now() - CLOUDSYNC_RECOVERY_DELAYED_AFTER);
        assert!(schedule.is_delayed("generation-1"));
        schedule.mark_progress("generation-1");
        assert!(!schedule.is_delayed("generation-1"));
        schedule.mark_failure("generation-1");
        assert!(schedule.is_delayed("generation-1"));
        schedule.mark_progress("generation-1");
        assert!(!schedule.is_delayed("generation-1"));
        schedule.claim("generation-1");
        assert!(schedule.is_active("generation-1"));

        schedule.cancel();
        assert!(!schedule.is_active("generation-1"));
    }

    async fn install_waiting_full_resync_task(
        runtime: &PluginDbRuntime,
    ) -> tokio::sync::oneshot::Receiver<()> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        let join_handle = tokio::spawn(async move {
            let _ = shutdown_rx.await;
            let _ = finished_tx.send(());
        });
        runtime
            .scheduled_cloudsync_full_resync
            .lock()
            .unwrap()
            .claim("test-generation");
        *runtime.cloudsync_full_resync_task.lock().await = Some(CloudsyncFullResyncTask {
            config: test_full_resync_config("test-token"),
            generation: "test-generation".to_string(),
            shutdown_tx: Some(shutdown_tx),
            join_handle,
        });
        finished_rx
    }

    #[tokio::test]
    async fn suspend_joins_full_resync_before_returning() {
        let runtime = PluginDbRuntime::new(std::sync::Arc::new(
            Db::connect_memory_plain().await.unwrap(),
        ));
        let finished = install_waiting_full_resync_task(&runtime).await;

        runtime.suspend_cloudsync().await.unwrap();

        tokio::time::timeout(std::time::Duration::from_secs(1), finished)
            .await
            .unwrap()
            .unwrap();
        assert!(runtime.cloudsync_full_resync_task.lock().await.is_none());
        assert!(
            !runtime
                .scheduled_cloudsync_full_resync
                .lock()
                .unwrap()
                .is_active("test-generation")
        );
    }

    #[tokio::test]
    async fn logout_joins_full_resync_before_returning() {
        let runtime = PluginDbRuntime::new(std::sync::Arc::new(
            Db::connect_memory_plain().await.unwrap(),
        ));
        let finished = install_waiting_full_resync_task(&runtime).await;

        runtime.logout_cloudsync(false).await.unwrap();

        tokio::time::timeout(std::time::Duration::from_secs(1), finished)
            .await
            .unwrap()
            .unwrap();
        assert!(runtime.cloudsync_full_resync_task.lock().await.is_none());
    }

    #[tokio::test]
    async fn dropping_runtime_aborts_full_resync_task() {
        struct DropSignal(Option<tokio::sync::oneshot::Sender<()>>);

        impl Drop for DropSignal {
            fn drop(&mut self) {
                if let Some(signal) = self.0.take() {
                    let _ = signal.send(());
                }
            }
        }

        let runtime = PluginDbRuntime::new(std::sync::Arc::new(
            Db::connect_memory_plain().await.unwrap(),
        ));
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (dropped_tx, dropped_rx) = tokio::sync::oneshot::channel();
        let join_handle = tokio::spawn(async move {
            let _shutdown_rx = shutdown_rx;
            let _drop_signal = DropSignal(Some(dropped_tx));
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        });
        *runtime.cloudsync_full_resync_task.lock().await = Some(CloudsyncFullResyncTask {
            config: test_full_resync_config("test-token"),
            generation: "test-generation".to_string(),
            shutdown_tx: Some(shutdown_tx),
            join_handle,
        });
        started_rx.await.unwrap();

        drop(runtime);

        tokio::time::timeout(std::time::Duration::from_secs(1), dropped_rx)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn poisoned_replica_recovery_requires_the_disposable_e2ee_table_only() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        hypr_db_app::claim_cloudsync_workspace(db.pool(), "workspace-1")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE cloudsync_table_settings (
               tbl_name TEXT NOT NULL,
               col_name TEXT NOT NULL,
               key TEXT NOT NULL,
               value TEXT,
               PRIMARY KEY (tbl_name, col_name, key)
             )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO cloudsync_table_settings (tbl_name, col_name, key, value)
             VALUES ('e2ee_records', '*', 'filter', 'workspace_id')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));

        require_disposable_cloudsync_replica(runtime.db.as_ref())
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO cloudsync_table_settings (tbl_name, col_name, key, value)
             VALUES ('sessions', '*', 'filter', 'workspace_id')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        assert!(
            require_disposable_cloudsync_replica(runtime.db.as_ref())
                .await
                .unwrap_err()
                .to_string()
                .contains("sessions")
        );
        let key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap()
        .workspace_key("workspace-1")
        .unwrap();
        assert!(
            prepare_cloudsync_poison_recovery(
                runtime.db.as_ref(),
                "workspace-1",
                "workspace-1",
                &key,
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("sessions")
        );
        assert_eq!(
            hypr_db_app::cloudsync_full_resync_generation(runtime.db.pool())
                .await
                .unwrap(),
            None
        );
        assert!(
            hypr_db_app::cloudsync_recovery_state(runtime.db.pool())
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn native_replica_logout_preserves_domain_rows_and_reinstalls_an_empty_filter() {
        let db = Db::connect_memory().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        hypr_db_app::set_cloudsync_personal_write_scope(db.pool(), "workspace-1")
            .await
            .unwrap();
        db.cloudsync_init(CLOUDSYNC_REPLICA_TABLE, None, None)
            .await
            .unwrap();
        db.cloudsync_set_filter(CLOUDSYNC_REPLICA_TABLE, CLOUDSYNC_WRITE_FILTER)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, title)
             VALUES ('session-1', 'workspace-1', 'Preserved')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload)
             VALUES ('record-1', 'workspace-1', 'ciphertext')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_local_state (
               record_id, workspace_id, table_name, row_id, field_name,
               revision, value_tag, payload_hash, writer_id, payload
             ) VALUES (
               'record-1', 'workspace-1', 'sessions', 'session-1', 'title',
               1, 'value-tag', 'payload-hash', 'writer-1', 'ciphertext'
             )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let local_changes_before: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records_cloudsync")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(local_changes_before > 0);
        let dirty_rows_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert!(dirty_rows_before > 0);
        let local_state_before: (String, String) = sqlx::query_as(
            "SELECT payload_hash, payload
             FROM e2ee_local_state
             WHERE record_id = 'record-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        db.cloudsync_network_logout().await.unwrap();

        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sessions")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );
        db.cloudsync_init(CLOUDSYNC_REPLICA_TABLE, None, None)
            .await
            .unwrap();
        db.cloudsync_set_filter(CLOUDSYNC_REPLICA_TABLE, CLOUDSYNC_WRITE_FILTER)
            .await
            .unwrap();

        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records_cloudsync")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*)
                 FROM cloudsync_table_settings
                 WHERE tbl_name = 'e2ee_records'
                   AND col_name = '*'
                   AND key = 'filter'
                   AND value = ?",
            )
            .bind(CLOUDSYNC_WRITE_FILTER)
            .fetch_one(db.pool())
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_dirty_rows")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            dirty_rows_before
        );
        assert_eq!(
            sqlx::query_as::<_, (String, String)>(
                "SELECT payload_hash, payload
                 FROM e2ee_local_state
                 WHERE record_id = 'record-1'",
            )
            .fetch_one(db.pool())
            .await
            .unwrap(),
            local_state_before
        );
    }

    #[tokio::test]
    async fn legacy_cutover_snapshots_local_state_before_initializing_the_witness() {
        let db = std::sync::Arc::new(Db::connect_memory().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        db.cloudsync_init("sessions", None, None).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-1', 'workspace-1', 'Legacy session')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_local_state")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );

        let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        runtime
            .set_e2ee_recovery_key("workspace-1", &recovery_key)
            .unwrap();
        let workspace_key = runtime.workspace_key("workspace-1").unwrap();
        let witness_state = InitiallyUninitializedWitness::default();
        let witness_server = MockServer::start().await;
        Mock::given(path("/sync/e2ee/witness/workspace-1"))
            .respond_with(witness_state.clone())
            .mount(&witness_server)
            .await;
        let witness = crate::e2ee_witness::E2eeWitnessClient::new(
            crate::CloudsyncE2eeWitness {
                endpoint: format!("{}/sync/e2ee/witness/workspace-1", witness_server.uri()),
                access_token: "access-token".to_string(),
            },
            "workspace-1",
        )
        .unwrap();

        runtime
            .prepare_e2ee_cutover_and_initialize_witness(&witness, &workspace_key)
            .await
            .unwrap();

        assert!(witness_state.initialized.load(Ordering::SeqCst));
        assert!(
            hypr_db_app::has_e2ee_local_state(db.pool(), "workspace-1")
                .await
                .unwrap()
        );
        assert!(
            !hypr_db_core::cloudsync_is_enabled_on(db.pool(), "sessions")
                .await
                .unwrap()
        );
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
    async fn failed_cloudsync_preflight_clears_new_credentials() {
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
            .prepare_cloudsync_config_fail_closed(hypr_db_core::CloudsyncRuntimeConfig {
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

    #[tokio::test]
    async fn cloudsync_status_reports_pending_e2ee_dirty_rows() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        runtime
            .set_e2ee_recovery_key("workspace-1", &recovery_key)
            .unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, title)
             VALUES ('session-1', 'workspace-1', 'Local edit')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let status = runtime.cloudsync_status().await.unwrap();

        assert_eq!(status["has_unsent_changes"], true);
    }

    #[tokio::test]
    async fn cloudsync_status_ignores_only_the_active_transcript() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        runtime
            .set_e2ee_recovery_key("workspace-1", &recovery_key)
            .unwrap();
        sqlx::query(
            "INSERT INTO transcripts (id, workspace_id, session_id, words_json)
             VALUES ('transcript-1', 'workspace-1', 'session-1', '[{\"text\":\"partial\"}]')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO app_settings (id, value_json)
             VALUES ('capture_lifecycle_pending:session-1', ?)",
        )
        .bind(
            serde_json::json!({
                "version": 1,
                "phase": "capturing",
                "sessionId": "session-1",
                "transcriptId": "transcript-1",
                "startedAt": 1_000,
                "createdAt": "2026-07-24T00:00:00.000Z",
                "audioOffsetMs": 0,
                "preserveExistingTranscript": false,
                "ownerUserId": "workspace-1",
                "memo": ""
            })
            .to_string(),
        )
        .execute(db.pool())
        .await
        .unwrap();

        let status = runtime.cloudsync_status().await.unwrap();
        assert_ne!(status["has_unsent_changes"], true);

        sqlx::query(
            "UPDATE app_settings
             SET value_json = json_set(value_json, '$.phase', 'finalizing')
             WHERE id = 'capture_lifecycle_pending:session-1'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let status = runtime.cloudsync_status().await.unwrap();
        assert_eq!(status["has_unsent_changes"], true);
    }

    #[tokio::test]
    async fn cloudsync_status_fails_fast_when_local_status_queries_are_busy() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
        let _held_connection = db.pool().acquire().await.unwrap();
        let started = std::time::Instant::now();

        let error = runtime.cloudsync_status().await.unwrap_err();

        assert!(matches!(
            error,
            crate::Error::Cloudsync(hypr_db_core::CloudsyncRuntimeError::LocalStatusBusy)
        ));
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }
}

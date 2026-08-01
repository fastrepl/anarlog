use std::collections::HashMap;

use anlg_db_core::Db;
#[cfg(test)]
use anlg_db_core::{DbOpenError, DbOpenOptions, DbStorage};
use anlg_db_execute::{DbExecutor, ProxyQueryMethod, ProxyQueryResult};
use anlg_db_reactive::{LiveQueryRuntime, QueryEventSink, SubscriptionRegistration};
use tauri::ipc::Channel;

use crate::{QueryEvent, Result, TransactionStatement};

mod e2ee_sync;
mod open;

pub(crate) use e2ee_sync::CloudsyncTokenConfiguration;
use e2ee_sync::E2eeSyncHook;
pub use open::open_app_db;
#[cfg(test)]
use open::{app_db_open_options, database_uses_cloudsync_schema, open_app_db_without_cloudsync};

const DEFAULT_CLOUDSYNC_INTERVAL_MS: u64 = 30_000;
const CLOUDSYNC_FULL_RESYNC_PROGRESS_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(200);
const CLOUDSYNC_FULL_RESYNC_RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
const CLOUDSYNC_RECOVERY_DELAYED_AFTER: std::time::Duration = std::time::Duration::from_secs(60);
const CLOUDSYNC_STATUS_POOL_RETURN_GRACE: std::time::Duration =
    std::time::Duration::from_millis(10);
const CLOUDSYNC_ACTIVITY_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
const CLOUDSYNC_AUTH_LOCK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
const CLOUDSYNC_REPLICA_TABLE: &str = "e2ee_records";
const E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT: i64 = 64;
const E2EE_CLOUDSYNC_WITNESS_REPAIR_BYTE_LIMIT: usize = 16 * 1024 * 1024;
const CLOUDSYNC_WRITE_FILTER: &str =
    "workspace_id IN (SELECT allowed_workspace_id FROM cloudsync_writable_workspaces)";
const CLOUDSYNC_CAPTURE_ACTIVITY: &str = "capture";

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

struct ExplicitRollbackTransaction {
    transaction: Option<sqlx::Transaction<'static, sqlx::Sqlite>>,
}

impl ExplicitRollbackTransaction {
    fn new(transaction: sqlx::Transaction<'static, sqlx::Sqlite>) -> Self {
        Self {
            transaction: Some(transaction),
        }
    }

    fn connection(&mut self) -> &mut sqlx::SqliteConnection {
        &mut *self
            .transaction
            .as_mut()
            .expect("transaction should be present")
    }

    async fn commit(mut self) -> std::result::Result<(), sqlx::Error> {
        self.transaction
            .take()
            .expect("transaction should be present")
            .commit()
            .await
    }

    async fn rollback(mut self) -> std::result::Result<(), sqlx::Error> {
        self.transaction
            .take()
            .expect("transaction should be present")
            .rollback()
            .await
    }
}

impl Drop for ExplicitRollbackTransaction {
    fn drop(&mut self) {
        let Some(transaction) = self.transaction.take() else {
            return;
        };

        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            tracing::error!("sqlite_transaction_cancelled_without_async_runtime");
            drop(transaction);
            return;
        };

        runtime.spawn(async move {
            if let Err(error) = transaction.rollback().await {
                tracing::error!(%error, "sqlite_cancelled_transaction_rollback_failed");
            }
        });
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
    cloudsync_control_operation: std::sync::Arc<tokio::sync::Mutex<()>>,
    cloudsync_activity_acquisition: tokio::sync::Mutex<()>,
    cloudsync_auth_generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
    cloudsync_auth_changed: std::sync::Arc<tokio::sync::Notify>,
    #[cfg(test)]
    pause_transaction_after_begin: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    transaction_started: tokio::sync::Notify,
}

struct CloudsyncFullResyncTask {
    #[cfg(test)]
    config: anlg_db_core::CloudsyncRuntimeConfig,
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
    Deferred,
    Complete,
}

#[derive(Clone, Copy)]
enum CloudsyncRecoveryCancellation {
    Shutdown,
    AuthChanged,
    Activity,
}

fn cloudsync_recovery_cancelled(cancelled: &std::sync::atomic::AtomicBool) -> bool {
    cancelled.load(std::sync::atomic::Ordering::Acquire)
}

#[derive(Clone, Copy)]
enum CloudsyncOperationCancellation<'a> {
    #[cfg(test)]
    None,
    Recovery(&'a std::sync::atomic::AtomicBool),
    Configuration(&'a crate::e2ee_witness::E2eeWitnessCancellation),
}

impl CloudsyncOperationCancellation<'_> {
    fn check(self) -> Result<()> {
        match self {
            #[cfg(test)]
            Self::None => Ok(()),
            Self::Recovery(cancelled) if cloudsync_recovery_cancelled(cancelled) => {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "CloudSync recovery cancelled",
                )
                .into())
            }
            Self::Recovery(_) => Ok(()),
            Self::Configuration(cancellation) => {
                cancellation.check()?;
                Ok(())
            }
        }
    }
}

fn cloudsync_recovery_step_delay(step: CloudsyncRecoveryStep) -> Option<std::time::Duration> {
    match step {
        CloudsyncRecoveryStep::Progressed => Some(CLOUDSYNC_FULL_RESYNC_PROGRESS_INTERVAL),
        CloudsyncRecoveryStep::Waiting => Some(CLOUDSYNC_FULL_RESYNC_RETRY_INTERVAL),
        CloudsyncRecoveryStep::Deferred => None,
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

    fn mark_activity_resumed(&mut self) {
        if self.generation.is_some() {
            self.last_progress_at = Some(std::time::Instant::now());
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
            cloudsync_control_operation: Default::default(),
            cloudsync_activity_acquisition: Default::default(),
            cloudsync_auth_generation: Default::default(),
            cloudsync_auth_changed: Default::default(),
            #[cfg(test)]
            pause_transaction_after_begin: Default::default(),
            #[cfg(test)]
            transaction_started: Default::default(),
        }
    }

    pub fn set_e2ee_recovery_key(
        &self,
        workspace_id: &str,
        recovery_key: &anlg_e2ee::RecoveryKey,
    ) -> Result<()> {
        self.e2ee_sync_hook
            .set_personal_workspace(workspace_id, recovery_key)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        Ok(())
    }

    pub fn pool(&self) -> &sqlx::SqlitePool {
        self.db.pool()
    }

    #[cfg(test)]
    pub(crate) fn pause_next_transaction_after_begin(&self) {
        self.pause_transaction_after_begin
            .store(true, std::sync::atomic::Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) async fn wait_for_transaction_after_begin(&self) {
        self.transaction_started.notified().await;
    }

    pub fn workspace_key(&self, workspace_id: &str) -> Option<anlg_e2ee::WorkspaceKey> {
        self.e2ee_sync_hook.workspace_key(workspace_id)
    }

    pub async fn synced_write_guard(&self) -> tokio::sync::RwLockReadGuard<'_, ()> {
        self.synced_write_barrier.read().await
    }

    async fn cloudsync_control_guard(&self) -> Result<tokio::sync::MutexGuard<'_, ()>> {
        let activity_changed = self.e2ee_sync_hook.activity_changed.notified();
        tokio::pin!(activity_changed);
        activity_changed.as_mut().enable();
        if self.e2ee_sync_hook.activity_paused() {
            return Err(crate::Error::CloudsyncActivityDeferred);
        }
        let guard = tokio::select! {
            biased;
            _ = &mut activity_changed => {
                return Err(crate::Error::CloudsyncActivityDeferred);
            }
            guard = self.cloudsync_control_operation.lock() => guard,
        };
        if self.e2ee_sync_hook.activity_paused() {
            return Err(crate::Error::CloudsyncActivityDeferred);
        }
        Ok(guard)
    }

    fn cloudsync_auth_generation(&self) -> u64 {
        self.cloudsync_auth_generation
            .load(std::sync::atomic::Ordering::Acquire)
    }

    pub(crate) fn begin_cloudsync_auth_configuration(&self) -> u64 {
        let generation = self
            .cloudsync_auth_generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
            .wrapping_add(1);
        self.cloudsync_auth_changed.notify_waiters();
        generation
    }

    fn invalidate_cloudsync_auth_generation(&self) {
        self.cloudsync_auth_generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        self.cloudsync_auth_changed.notify_waiters();
    }

    fn ensure_cloudsync_auth_generation(&self, generation: u64) -> Result<()> {
        if self.cloudsync_auth_generation() != generation {
            return Err(crate::Error::CloudsyncConfigurationCancelled);
        }
        Ok(())
    }

    fn ensure_cloudsync_configuration_active(
        &self,
        generation: u64,
        cancellation: &crate::e2ee_witness::E2eeWitnessCancellation,
    ) -> Result<()> {
        self.ensure_cloudsync_auth_generation(generation)?;
        cancellation.check()?;
        Ok(())
    }

    async fn wait_until_cloudsync_auth_generation_changes(&self, generation: u64) {
        wait_until_cloudsync_auth_generation_changes(
            self.cloudsync_auth_generation.as_ref(),
            self.cloudsync_auth_changed.as_ref(),
            generation,
        )
        .await;
    }

    pub async fn begin_cloudsync_activity(&self, activity: String, key: String) -> Result<()> {
        self.begin_cloudsync_activity_with_timeout(activity, key, CLOUDSYNC_ACTIVITY_DRAIN_TIMEOUT)
            .await
    }

    async fn begin_cloudsync_activity_with_timeout(
        &self,
        activity: String,
        key: String,
        drain_timeout: std::time::Duration,
    ) -> Result<()> {
        let activity = normalized_cloudsync_activity_part(activity, "activity")?;
        let key = normalized_cloudsync_activity_part(key, "key")?;
        let _acquisition = self.cloudsync_activity_acquisition.lock().await;
        if self.e2ee_sync_hook.has_activity_lease(&activity, &key) {
            return Ok(());
        }
        self.e2ee_sync_hook
            .begin_activity(activity.clone(), key.clone());
        let drain = async {
            let sync_idle = self.db.cloudsync_wait_for_sync_idle();
            tokio::pin!(sync_idle);
            let mut interrupt_interval =
                tokio::time::interval(std::time::Duration::from_millis(25));
            loop {
                tokio::select! {
                    biased;
                    () = &mut sync_idle => break,
                    _ = interrupt_interval.tick() => {
                        self.db.cloudsync_interrupt_sync();
                    }
                }
            }
            drop(self.cloudsync_control_operation.lock().await);
        };
        if tokio::time::timeout(drain_timeout, drain).await.is_err() {
            let lease_active = self.e2ee_sync_hook.has_activity_lease(&activity, &key);
            if lease_active {
                self.finish_cloudsync_activity(&activity, &key);
            } else {
                return Err(std::io::Error::other(
                    "CloudSync activity ended before synchronization became idle",
                )
                .into());
            }
            tracing::error!(
                activity,
                key,
                timeout_ms = drain_timeout.as_millis(),
                "CloudSync activity could not drain the in-flight operation",
            );
            return Err(crate::Error::CloudsyncActivityDrainTimeout);
        }
        if !self.e2ee_sync_hook.has_activity_lease(&activity, &key) {
            return Err(std::io::Error::other(
                "CloudSync activity ended before synchronization became idle",
            )
            .into());
        }
        Ok(())
    }

    pub async fn end_cloudsync_activity(&self, activity: String, key: String) -> Result<()> {
        let activity = normalized_cloudsync_activity_part(activity, "activity")?;
        let key = normalized_cloudsync_activity_part(key, "key")?;
        self.finish_cloudsync_activity(&activity, &key);
        Ok(())
    }

    fn finish_cloudsync_activity(&self, activity: &str, key: &str) {
        if !self.e2ee_sync_hook.end_activity(activity, key) {
            return;
        }
        self.scheduled_cloudsync_full_resync
            .lock()
            .unwrap()
            .mark_activity_resumed();
        self.e2ee_sync_hook.notify_activity_changed();
        self.db.cloudsync_request_sync();
    }

    async fn ensure_app_schema(&self) -> Result<()> {
        self.schema_ready
            .get_or_try_init(|| async { anlg_db_app::prepare_schema(self.db.as_ref()).await })
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
        let mut transaction =
            ExplicitRollbackTransaction::new(self.db.pool().begin_with("BEGIN IMMEDIATE").await?);
        #[cfg(test)]
        if self
            .pause_transaction_after_begin
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            self.transaction_started.notify_one();
            std::future::pending::<()>().await;
        }
        let mut rows_affected = Vec::with_capacity(statements.len());

        for (statement_index, statement) in statements.into_iter().enumerate() {
            let result = match bind_params(
                sqlx::query(sqlx::AssertSqlSafe(statement.sql.as_str())),
                &statement.params,
            )
            .execute(transaction.connection())
            .await
            {
                Ok(result) => result,
                Err(error) => {
                    if let Err(rollback_error) = transaction.rollback().await {
                        tracing::error!(
                            %rollback_error,
                            "sqlite_failed_transaction_rollback_failed"
                        );
                    }
                    return Err(error.into());
                }
            };
            let actual = result.rows_affected();
            if let Some(expected) = statement.expected_rows_affected
                && actual != expected
            {
                let error = crate::Error::UnexpectedRowsAffected {
                    statement_index,
                    expected,
                    actual,
                };
                if let Err(rollback_error) = transaction.rollback().await {
                    tracing::error!(
                        %rollback_error,
                        "sqlite_mismatched_transaction_rollback_failed"
                    );
                }
                return Err(error);
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

    pub async fn unsubscribe(&self, subscription_id: &str) -> anlg_db_reactive::Result<()> {
        self.live_query_runtime.unsubscribe(subscription_id).await
    }

    pub async fn configure_cloudsync(&self, config_json: String) -> Result<()> {
        let _control_operation = self.cloudsync_control_guard().await?;
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
        workspace_projection: Option<anlg_db_app::CloudsyncWorkspaceProjection>,
        e2ee_witness: crate::CloudsyncE2eeWitness,
    ) -> Result<crate::CloudsyncTokenConfigurationResult> {
        let auth_generation = self.begin_cloudsync_auth_configuration();
        self.configure_cloudsync_token_with_projection_at_generation(
            CloudsyncTokenConfiguration::new(
                database_id,
                token,
                account_user_id,
                workspace_projection,
                e2ee_witness,
            ),
            None,
            auth_generation,
        )
        .await
    }

    pub(crate) async fn configure_cloudsync_token_with_projection_at_generation(
        &self,
        configuration: CloudsyncTokenConfiguration,
        recovery_key: Option<(String, anlg_e2ee::RecoveryKey)>,
        auth_generation: u64,
    ) -> Result<crate::CloudsyncTokenConfigurationResult> {
        let _control_operation = self.cloudsync_control_guard().await?;
        let configuration_cancellation = crate::e2ee_witness::E2eeWitnessCancellation::default();
        let mut attempt_started = false;
        let result = {
            let configuration = async {
                self.ensure_cloudsync_configuration_active(
                    auth_generation,
                    &configuration_cancellation,
                )?;
                attempt_started = true;
                self.cancel_cloudsync_full_resync().await;
                self.ensure_cloudsync_configuration_active(
                    auth_generation,
                    &configuration_cancellation,
                )?;
                self.db.cloudsync_stop().await?;
                self.ensure_cloudsync_configuration_active(
                    auth_generation,
                    &configuration_cancellation,
                )?;
                if let Some((workspace_id, recovery_key)) = recovery_key {
                    self.set_e2ee_recovery_key(&workspace_id, &recovery_key)?;
                    self.ensure_cloudsync_configuration_active(
                        auth_generation,
                        &configuration_cancellation,
                    )?;
                }
                self.configure_cloudsync_token_with_projection_inner(
                    configuration,
                    auth_generation,
                    &configuration_cancellation,
                )
                .await
            };
            tokio::pin!(configuration);
            let selected: std::result::Result<
                Result<crate::CloudsyncTokenConfigurationResult>,
                crate::Error,
            > = tokio::select! {
                biased;
                _ = self.wait_until_cloudsync_auth_generation_changes(auth_generation) => {
                    Err(crate::Error::CloudsyncConfigurationCancelled)
                }
                _ = self.e2ee_sync_hook.wait_until_activity_paused() => {
                    Err(crate::Error::CloudsyncActivityDeferred)
                }
                result = &mut configuration => Ok(result),
            };
            match selected {
                Ok(result) => result,
                Err(error) => {
                    configuration_cancellation.cancel();
                    let mut interrupt_interval =
                        tokio::time::interval(std::time::Duration::from_millis(25));
                    loop {
                        tokio::select! {
                            biased;
                            _ = &mut configuration => break,
                            _ = interrupt_interval.tick() => {
                                self.db.cloudsync_interrupt_sync();
                            }
                        }
                    }
                    let sync_idle = self.db.cloudsync_wait_for_sync_idle();
                    tokio::pin!(sync_idle);
                    loop {
                        tokio::select! {
                            biased;
                            () = &mut sync_idle => break,
                            _ = interrupt_interval.tick() => {
                                self.db.cloudsync_interrupt_sync();
                            }
                        }
                    }
                    Err(error)
                }
            }
        };
        let result = match result {
            Ok(result) => self
                .ensure_cloudsync_configuration_active(auth_generation, &configuration_cancellation)
                .map(|()| result),
            Err(error) => Err(error),
        };
        let should_fail_closed =
            attempt_started || self.cloudsync_auth_generation() == auth_generation;
        if should_fail_closed
            && (result.is_err()
                || matches!(
                    &result,
                    Ok(crate::CloudsyncTokenConfigurationResult::AccountMismatch)
                ))
        {
            self.cancel_cloudsync_full_resync().await;
            let _ = self.db.cloudsync_suspend().await;
            self.e2ee_sync_hook.clear();
        }
        result
    }

    async fn configure_cloudsync_token_with_projection_inner(
        &self,
        configuration: CloudsyncTokenConfiguration,
        auth_generation: u64,
        cancellation: &crate::e2ee_witness::E2eeWitnessCancellation,
    ) -> Result<crate::CloudsyncTokenConfigurationResult> {
        let CloudsyncTokenConfiguration {
            database_id,
            token,
            account_user_id,
            workspace_projection,
            e2ee_witness,
        } = configuration;
        cancellation.check()?;
        if !self.db.cloudsync_enabled() {
            return Err(anlg_db_core::CloudsyncRuntimeError::Unavailable.into());
        }

        if workspace_projection
            .as_ref()
            .is_some_and(|projection| projection.account_user_id != account_user_id)
        {
            return Err(anlg_db_app::CloudsyncWorkspaceError::InvalidWorkspaceProjection.into());
        }
        if let Some(projection) = workspace_projection.as_ref() {
            anlg_db_app::validate_cloudsync_workspace_projection(projection)?;
        }

        self.ensure_legacy_migration_verified().await?;
        self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;

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
            .claim_cloudsync_workspace(account_user_id.clone(), cancellation)
            .await?
        {
            return Ok(crate::CloudsyncTokenConfigurationResult::AccountMismatch);
        }
        self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;

        if workspace_projection.is_some() {
            self.db.cloudsync_suspend().await?;
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
        }
        let witness =
            crate::e2ee_witness::E2eeWitnessClient::new(e2ee_witness, personal_workspace_id)?;
        let key = self
            .e2ee_sync_hook
            .workspace_key(personal_workspace_id)
            .ok_or(crate::Error::E2eeIdentityRequired)?;
        self.prepare_e2ee_cutover_and_initialize_witness(&witness, &key, cancellation)
            .await?;
        self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
        self.e2ee_sync_hook.set_witness(witness);
        let config = anlg_db_core::CloudsyncRuntimeConfig {
            connection_string: database_id,
            auth: anlg_db_core::CloudsyncAuth::Token { token },
            tables: anlg_db_app::cloudsync_table_registry().to_vec(),
            sync_interval_ms: DEFAULT_CLOUDSYNC_INTERVAL_MS,
            wait_ms: Some(5_000),
            max_retries: Some(3),
        };
        let write_filter_installed = match workspace_projection.as_ref() {
            Some(projection) => {
                let installed = anlg_db_app::cloudsync_write_filter_installed(
                    self.db.pool(),
                    &projection.personal_workspace_id,
                )
                .await?;
                self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
                if installed {
                    let filters_match = self.cloudsync_write_filters_match().await?;
                    self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
                    filters_match
                } else {
                    false
                }
            }
            None => true,
        };
        self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;

        let reconciliation = match workspace_projection.as_ref() {
            Some(projection) => {
                self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
                let _write_guard = self.synced_write_barrier.write().await;
                self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
                let reconciliation =
                    match anlg_db_app::stage_cloudsync_workspace_reconciliation_cancellable(
                        self.db.pool(),
                        projection,
                        || cancellation.is_cancelled(),
                    )
                    .await
                    {
                        Ok(reconciliation) => reconciliation,
                        Err(anlg_db_app::CloudsyncWorkspaceError::ProjectionCancelled) => {
                            return Err(crate::Error::CloudsyncConfigurationCancelled);
                        }
                        Err(error) => return Err(error.into()),
                    };
                self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
                Some(reconciliation)
            }
            None => None,
        };
        if let Some(projection) = workspace_projection.as_ref() {
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            anlg_db_app::set_cloudsync_personal_write_scope(
                self.db.pool(),
                &projection.personal_workspace_id,
            )
            .await?;
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            let _write_guard = self.synced_write_barrier.write().await;
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            let requires_full_resync = reconciliation
                .as_ref()
                .is_some_and(|plan| plan.requires_full_resync())
                || !write_filter_installed;
            match anlg_db_app::commit_cloudsync_workspace_projection_cancellable(
                self.db.pool(),
                projection,
                requires_full_resync,
                || cancellation.is_cancelled(),
            )
            .await
            {
                Ok(_) => {}
                Err(anlg_db_app::CloudsyncWorkspaceError::ProjectionCancelled) => {
                    return Err(crate::Error::CloudsyncConfigurationCancelled);
                }
                Err(error) => return Err(error.into()),
            }
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
        }
        self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;

        if let Some(generation) =
            anlg_db_app::cloudsync_full_resync_generation(self.db.pool()).await?
        {
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            anlg_db_app::ensure_cloudsync_recovery_state(
                self.db.pool(),
                &generation,
                &account_user_id,
                personal_workspace_id,
                &key,
            )
            .await?;
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            self.db.cloudsync_suspend().await?;
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            self.db
                .cloudsync_prepare_manual_transport(config.clone())
                .await?;
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            if anlg_db_app::cloudsync_recovery_state(self.db.pool())
                .await?
                .is_some_and(|state| {
                    state.generation == generation
                        && state.phase == anlg_db_app::CloudsyncRecoveryPhase::NeedFirstLogout
                })
            {
                self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
                discard_cloudsync_recovery_replica(
                    self.db.as_ref(),
                    &config,
                    &generation,
                    CloudsyncOperationCancellation::Configuration(cancellation),
                )
                .await?;
                self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            }
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            self.schedule_cloudsync_full_resync(generation, config, auth_generation)
                .await;
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
        } else {
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            let (pending_fits, pending_chunks, pending_rows, pending_bytes) = self
                .prepare_cloudsync_config_fail_closed(config.clone(), cancellation)
                .await?;
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            if pending_fits {
                self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
                self.db.cloudsync_resume_prepared_transport().await?;
                self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            } else {
                tracing::warn!(
                    chunks = pending_chunks,
                    rows = pending_rows,
                    bytes = pending_bytes,
                    "recovering an oversized CloudSync outbox before it reaches the server",
                );
                self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
                let generation = prepare_cloudsync_poison_recovery(
                    self.db.as_ref(),
                    &account_user_id,
                    personal_workspace_id,
                    &key,
                    CloudsyncOperationCancellation::Configuration(cancellation),
                )
                .await?;
                self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
                discard_cloudsync_recovery_replica(
                    self.db.as_ref(),
                    &config,
                    &generation,
                    CloudsyncOperationCancellation::Configuration(cancellation),
                )
                .await?;
                self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
                self.schedule_cloudsync_full_resync(generation, config, auth_generation)
                    .await;
                self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            }
        }
        Ok(crate::CloudsyncTokenConfigurationResult::Configured)
    }

    pub async fn bind_cloudsync_account(&self, account_user_id: String) -> Result<bool> {
        self.bind_cloudsync_account_with_lock_timeout(account_user_id, CLOUDSYNC_AUTH_LOCK_TIMEOUT)
            .await
    }

    async fn bind_cloudsync_account_with_lock_timeout(
        &self,
        account_user_id: String,
        lock_timeout: std::time::Duration,
    ) -> Result<bool> {
        let (_control_operation, _write_guard) = tokio::time::timeout(lock_timeout, async {
            let control_operation = self.cloudsync_control_operation.lock().await;
            let write_guard = self.synced_write_barrier.write().await;
            (control_operation, write_guard)
        })
        .await
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "CloudSync account binding lock preflight timed out",
            )
        })?;
        self.ensure_app_schema().await?;
        match anlg_db_app::bind_cloudsync_account(self.db.pool(), &account_user_id).await {
            Ok(()) => Ok(true),
            Err(anlg_db_app::CloudsyncWorkspaceError::AccountMismatch) => {
                self.db.cloudsync_suspend().await?;
                Ok(false)
            }
            Err(error) => {
                let _ = self.db.cloudsync_suspend().await;
                Err(error.into())
            }
        }
    }

    async fn claim_cloudsync_workspace(
        &self,
        account_user_id: String,
        cancellation: &crate::e2ee_witness::E2eeWitnessCancellation,
    ) -> Result<bool> {
        cancellation.check()?;
        self.ensure_app_schema().await?;
        cancellation.check()?;
        let claimed =
            anlg_db_app::cloudsync_workspace_is_claimed_by(self.db.pool(), &account_user_id).await;
        cancellation.check()?;
        match claimed {
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
        cancellation.check()?;
        match anlg_db_app::claim_cloudsync_workspace_cancellable(
            self.db.pool(),
            &account_user_id,
            || cancellation.is_cancelled(),
        )
        .await
        {
            Ok(()) => Ok(true),
            Err(anlg_db_app::CloudsyncWorkspaceError::ClaimCancelled) => {
                Err(crate::Error::CloudsyncConfigurationCancelled)
            }
            Err(error) if is_permanent_cloudsync_workspace_rejection(&error) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    async fn prepare_cloudsync_config_fail_closed(
        &self,
        config: anlg_db_core::CloudsyncRuntimeConfig,
        cancellation: &crate::e2ee_witness::E2eeWitnessCancellation,
    ) -> Result<(bool, u32, u64, u64)> {
        let result: Result<(bool, u32, u64, u64)> = async {
            cancellation.check()?;
            self.db.cloudsync_stop().await?;
            cancellation.check()?;
            self.db.cloudsync_prepare_manual_transport(config).await?;
            cancellation.check()?;
            let batch = self.db.cloudsync_manual_pending_payload_batch().await?;
            cancellation.check()?;
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

    async fn prepare_e2ee_cutover(
        &self,
        cancellation: &crate::e2ee_witness::E2eeWitnessCancellation,
    ) -> Result<()> {
        cancellation.check()?;
        let legacy_cutover_required = self.legacy_e2ee_cutover_required().await?;
        cancellation.check()?;
        if !legacy_cutover_required {
            return Ok(());
        }

        self.e2ee_sync_hook
            .prepare_local_snapshot(self.db.pool(), cancellation)
            .await
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        cancellation.check()?;

        let _write_guard = self.synced_write_barrier.write().await;
        cancellation.check()?;
        for table_name in anlg_db_app::E2EE_DOMAIN_TABLES {
            let enabled = anlg_db_core::cloudsync_is_enabled_on(self.db.pool(), table_name)
                .await
                .map_err(anlg_db_core::CloudsyncRuntimeError::from)?;
            cancellation.check()?;
            if enabled {
                self.db
                    .cloudsync_cleanup(table_name)
                    .await
                    .map_err(anlg_db_core::CloudsyncRuntimeError::from)?;
                cancellation.check()?;
            }
        }
        Ok(())
    }

    async fn prepare_e2ee_cutover_and_initialize_witness(
        &self,
        witness: &crate::e2ee_witness::E2eeWitnessClient,
        key: &anlg_e2ee::WorkspaceKey,
        cancellation: &crate::e2ee_witness::E2eeWitnessCancellation,
    ) -> Result<()> {
        self.prepare_e2ee_cutover(cancellation).await?;
        cancellation.check()?;
        witness
            .initialize_cancellable(self.db.pool(), key, cancellation)
            .await?;
        cancellation.check()?;
        Ok(())
    }

    async fn legacy_e2ee_cutover_required(&self) -> Result<bool> {
        for table_name in anlg_db_app::E2EE_DOMAIN_TABLES {
            if anlg_db_core::cloudsync_is_enabled_on(self.db.pool(), table_name)
                .await
                .map_err(anlg_db_core::CloudsyncRuntimeError::from)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn schedule_cloudsync_full_resync(
        &self,
        generation: String,
        config: anlg_db_core::CloudsyncRuntimeConfig,
        auth_generation: u64,
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
        let control_operation = std::sync::Arc::clone(&self.cloudsync_control_operation);
        let cloudsync_auth_generation = std::sync::Arc::clone(&self.cloudsync_auth_generation);
        let cloudsync_auth_changed = std::sync::Arc::clone(&self.cloudsync_auth_changed);
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        let join_handle = tokio::spawn(async move {
            let mut clean_receive_attempt_started = false;
            loop {
                if cloudsync_auth_generation.load(std::sync::atomic::Ordering::Acquire)
                    != auth_generation
                {
                    return;
                }
                if !scheduled.lock().unwrap().is_active(&generation) {
                    return;
                }
                if e2ee_sync_hook.activity_paused() {
                    tokio::select! {
                        biased;
                        _ = &mut shutdown_rx => return,
                        _ = e2ee_sync_hook.wait_until_activity_resumed() => {}
                    }
                    continue;
                }

                let recovery_cancelled = std::sync::atomic::AtomicBool::new(false);
                let witness_cancellation = crate::e2ee_witness::E2eeWitnessCancellation::default();
                let mut recovery_step = Box::pin(async {
                    #[cfg(test)]
                    e2ee_sync_hook
                        .full_resync_control_waits
                        .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                    let _control_operation = tokio::select! {
                        biased;
                        _ = witness_cancellation.cancelled() => {
                            return Ok(CloudsyncRecoveryStep::Deferred);
                        }
                        control_operation = control_operation.lock() => control_operation,
                    };
                    if cloudsync_recovery_cancelled(&recovery_cancelled)
                        || e2ee_sync_hook.activity_paused()
                    {
                        return Ok(CloudsyncRecoveryStep::Deferred);
                    }
                    let state = anlg_db_app::cloudsync_recovery_state(db.pool())
                        .await?
                        .ok_or_else(|| {
                            std::io::Error::other(
                                "CloudSync recovery state disappeared while resync is pending",
                            )
                        })?;
                    if cloudsync_recovery_cancelled(&recovery_cancelled) {
                        return Ok(CloudsyncRecoveryStep::Deferred);
                    }
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
                    if cloudsync_recovery_cancelled(&recovery_cancelled) {
                        return Ok(CloudsyncRecoveryStep::Deferred);
                    }

                    match state.phase {
                        anlg_db_app::CloudsyncRecoveryPhase::NeedFirstLogout => {
                            discard_cloudsync_recovery_replica(
                                db.as_ref(),
                                &config,
                                &generation,
                                CloudsyncOperationCancellation::Recovery(&recovery_cancelled),
                            )
                            .await?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert => {
                            anlg_db_app::insert_cloudsync_recovery_barrier(db.pool(), &state, &key)
                                .await?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if !anlg_db_app::advance_cloudsync_recovery_phase(
                                db.pool(),
                                &generation,
                                anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert,
                                anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm,
                            )
                            .await?
                            {
                                return Err(
                                    anlg_db_app::CloudsyncWorkspaceError::RecoveryConflict.into()
                                );
                            }
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm => {
                            match flush_manual_cloudsync_pending(db.as_ref(), &recovery_cancelled)
                                .await?
                            {
                                CloudsyncPendingFlush::Progressed => {
                                    return Ok(CloudsyncRecoveryStep::Progressed);
                                }
                                CloudsyncPendingFlush::Waiting => {
                                    return Ok(CloudsyncRecoveryStep::Waiting);
                                }
                                CloudsyncPendingFlush::Empty => {}
                            }
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if !anlg_db_app::cloudsync_recovery_barrier_is_exact(
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
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if !anlg_db_app::advance_cloudsync_recovery_phase(
                                db.pool(),
                                &generation,
                                anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm,
                                anlg_db_app::CloudsyncRecoveryPhase::NeedCleanReceive,
                            )
                            .await?
                            {
                                return Err(
                                    anlg_db_app::CloudsyncWorkspaceError::RecoveryConflict.into()
                                );
                            }
                            clean_receive_attempt_started = false;
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        anlg_db_app::CloudsyncRecoveryPhase::NeedCleanReceive => {
                            if !clean_receive_attempt_started {
                                require_disposable_cloudsync_replica(
                                    db.as_ref(),
                                    CloudsyncOperationCancellation::Recovery(&recovery_cancelled),
                                )
                                .await?;
                                if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                    return Ok(CloudsyncRecoveryStep::Deferred);
                                }
                                db.cloudsync_logout(true).await?;
                                if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                    return Ok(CloudsyncRecoveryStep::Deferred);
                                }
                                prepare_manual_cloudsync_recovery_transport(
                                    db.as_ref(),
                                    &config,
                                    CloudsyncOperationCancellation::Recovery(&recovery_cancelled),
                                )
                                .await?;
                                if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                    return Ok(CloudsyncRecoveryStep::Deferred);
                                }
                                db.cloudsync_network_reset_receive_version()
                                    .await
                                    .map_err(anlg_db_core::CloudsyncRuntimeError::from)?;
                                if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                    return Ok(CloudsyncRecoveryStep::Deferred);
                                }
                                if !anlg_db_app::cloudsync_encrypted_replica_is_empty(db.pool())
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
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            witness
                                .refresh_cancellable(db.pool(), &key, &witness_cancellation)
                                .await?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            let apply = anlg_db_app::apply_received_e2ee_replica_changes_with_witness_cancellable(
                                db.pool(),
                                &e2ee_sync_hook.snapshot(),
                                false,
                                || cloudsync_recovery_cancelled(&recovery_cancelled),
                            )
                            .await
                            .map_err(|error| std::io::Error::other(error.to_string()))?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            let barrier_is_exact =
                                anlg_db_app::cloudsync_recovery_barrier_is_exact(
                                    db.pool(),
                                    &state,
                                    &key,
                                )
                                .await?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if cloudsync_recovery_snapshot_ready(barrier_is_exact, &result) {
                                if !anlg_db_app::advance_cloudsync_recovery_phase(
                                    db.pool(),
                                    &generation,
                                    anlg_db_app::CloudsyncRecoveryPhase::NeedCleanReceive,
                                    anlg_db_app::CloudsyncRecoveryPhase::NeedWitnessRepair,
                                )
                                .await?
                                {
                                    return Err(
                                        anlg_db_app::CloudsyncWorkspaceError::RecoveryConflict
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
                        anlg_db_app::CloudsyncRecoveryPhase::NeedWitnessRepair => {
                            match flush_manual_cloudsync_pending(db.as_ref(), &recovery_cancelled)
                                .await?
                            {
                                CloudsyncPendingFlush::Progressed => {
                                    return Ok(CloudsyncRecoveryStep::Progressed);
                                }
                                CloudsyncPendingFlush::Waiting => {
                                    return Ok(CloudsyncRecoveryStep::Waiting);
                                }
                                CloudsyncPendingFlush::Empty => {}
                            }
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }

                            witness
                                .refresh_cancellable(db.pool(), &key, &witness_cancellation)
                                .await?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            witness
                                .publish_and_refresh_cancellable(
                                    db.pool(),
                                    &key,
                                    &witness_cancellation,
                                )
                                .await?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            let keys = e2ee_sync_hook.snapshot();
                            let repair =
                                anlg_db_app::repair_e2ee_replica_from_witness_bounded_cancellable(
                                    db.pool(),
                                    &keys,
                                    true,
                                    E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT,
                                    E2EE_CLOUDSYNC_WITNESS_REPAIR_BYTE_LIMIT,
                                    || cloudsync_recovery_cancelled(&recovery_cancelled),
                                )
                                .await
                                .map_err(|error| std::io::Error::other(error.to_string()))?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            let apply = anlg_db_app::apply_received_e2ee_replica_changes_with_witness_cancellable(
                                db.pool(),
                                &keys,
                                false,
                                || cloudsync_recovery_cancelled(&recovery_cancelled),
                            )
                            .await
                            .map_err(|error| std::io::Error::other(error.to_string()))?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if repair.repaired_records > 0 || apply.applied_fields > 0 {
                                return Ok(CloudsyncRecoveryStep::Progressed);
                            }
                            if repair.remaining || apply.remaining_replica_changes {
                                return Ok(CloudsyncRecoveryStep::Waiting);
                            }

                            witness
                                .refresh_cancellable(db.pool(), &key, &witness_cancellation)
                                .await?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if anlg_db_app::has_pending_e2ee_witness_repairs_cancellable(
                                db.pool(),
                                &keys,
                                true,
                                || cloudsync_recovery_cancelled(&recovery_cancelled),
                            )
                            .await
                            .map_err(|error| std::io::Error::other(error.to_string()))?
                            {
                                return Ok(CloudsyncRecoveryStep::Waiting);
                            }
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }

                            let local = anlg_db_app::encrypt_e2ee_replica_changes_bounded_deferring_active_captures_cancellable(
                                    db.pool(),
                                    &keys,
                                    E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT,
                                    || cloudsync_recovery_cancelled(&recovery_cancelled),
                                )
                                .await
                                .map_err(|error| std::io::Error::other(error.to_string()))?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if local.encrypted_fields > 0 {
                                witness
                                    .publish_and_refresh_cancellable(
                                        db.pool(),
                                        &key,
                                        &witness_cancellation,
                                    )
                                    .await?;
                                if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                    return Ok(CloudsyncRecoveryStep::Deferred);
                                }
                                return Ok(CloudsyncRecoveryStep::Progressed);
                            }
                            if anlg_db_app::has_pending_e2ee_dirty_rows_deferring_active_captures(
                                db.pool(),
                                &keys,
                            )
                            .await
                            .map_err(|error| std::io::Error::other(error.to_string()))?
                            {
                                return Ok(CloudsyncRecoveryStep::Waiting);
                            }
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }

                            witness
                                .refresh_cancellable(db.pool(), &key, &witness_cancellation)
                                .await?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if anlg_db_app::has_pending_e2ee_witness_repairs_cancellable(
                                db.pool(),
                                &keys,
                                true,
                                || cloudsync_recovery_cancelled(&recovery_cancelled),
                            )
                            .await
                            .map_err(|error| std::io::Error::other(error.to_string()))?
                            {
                                return Ok(CloudsyncRecoveryStep::Waiting);
                            }
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if !matches!(
                                flush_manual_cloudsync_pending(db.as_ref(), &recovery_cancelled)
                                    .await?,
                                CloudsyncPendingFlush::Empty
                            ) {
                                return Ok(CloudsyncRecoveryStep::Waiting);
                            }
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if !anlg_db_app::advance_cloudsync_recovery_phase(
                                db.pool(),
                                &generation,
                                anlg_db_app::CloudsyncRecoveryPhase::NeedWitnessRepair,
                                anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierCleanup,
                            )
                            .await?
                            {
                                return Err(
                                    anlg_db_app::CloudsyncWorkspaceError::RecoveryConflict.into()
                                );
                            }
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierCleanup => {
                            match flush_manual_cloudsync_pending(db.as_ref(), &recovery_cancelled)
                                .await?
                            {
                                CloudsyncPendingFlush::Progressed => {
                                    return Ok(CloudsyncRecoveryStep::Progressed);
                                }
                                CloudsyncPendingFlush::Waiting => {
                                    return Ok(CloudsyncRecoveryStep::Waiting);
                                }
                                CloudsyncPendingFlush::Empty => {}
                            }
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if anlg_db_app::cloudsync_recovery_barrier_is_exact(
                                db.pool(),
                                &state,
                                &key,
                            )
                            .await?
                            {
                                if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                    return Ok(CloudsyncRecoveryStep::Deferred);
                                }
                                anlg_db_app::delete_cloudsync_recovery_barrier(
                                    db.pool(),
                                    &state,
                                    &key,
                                )
                                .await?;
                                if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                    return Ok(CloudsyncRecoveryStep::Deferred);
                                }
                                return Ok(CloudsyncRecoveryStep::Progressed);
                            }
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if !anlg_db_app::advance_cloudsync_recovery_phase(
                                db.pool(),
                                &generation,
                                anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierCleanup,
                                anlg_db_app::CloudsyncRecoveryPhase::NeedTransportResume,
                            )
                            .await?
                            {
                                return Err(
                                    anlg_db_app::CloudsyncWorkspaceError::RecoveryConflict.into()
                                );
                            }
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        anlg_db_app::CloudsyncRecoveryPhase::NeedTransportResume => {
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            db.cloudsync_resume_prepared_transport().await?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if !anlg_db_app::complete_cloudsync_recovery(db.pool(), &generation)
                                .await?
                            {
                                return Err(
                                    anlg_db_app::CloudsyncWorkspaceError::RecoveryConflict.into()
                                );
                            }
                            Ok(CloudsyncRecoveryStep::Complete)
                        }
                    }
                });
                let selected: std::result::Result<
                    Result<CloudsyncRecoveryStep>,
                    CloudsyncRecoveryCancellation,
                > = tokio::select! {
                    biased;
                    _ = &mut shutdown_rx => {
                        Err(CloudsyncRecoveryCancellation::Shutdown)
                    },
                    _ = wait_until_cloudsync_auth_generation_changes(
                        cloudsync_auth_generation.as_ref(),
                        cloudsync_auth_changed.as_ref(),
                        auth_generation,
                    ) => {
                        Err(CloudsyncRecoveryCancellation::AuthChanged)
                    },
                    _ = e2ee_sync_hook.wait_until_activity_paused() => {
                        Err(CloudsyncRecoveryCancellation::Activity)
                    },
                    step = &mut recovery_step => Ok(step),
                };
                let step = match selected {
                    Ok(step) => step,
                    Err(cancellation) => {
                        recovery_cancelled.store(true, std::sync::atomic::Ordering::Release);
                        witness_cancellation.cancel();
                        let mut interrupt_interval =
                            tokio::time::interval(std::time::Duration::from_millis(25));
                        let drained_step = loop {
                            tokio::select! {
                                biased;
                                step = &mut recovery_step => break step,
                                _ = interrupt_interval.tick() => {
                                    db.cloudsync_interrupt_sync();
                                }
                            }
                        };
                        drop(recovery_step);
                        let sync_idle = db.cloudsync_wait_for_sync_idle();
                        tokio::pin!(sync_idle);
                        let mut interrupt_interval =
                            tokio::time::interval(std::time::Duration::from_millis(25));
                        loop {
                            tokio::select! {
                                biased;
                                () = &mut sync_idle => break,
                                _ = interrupt_interval.tick() => {
                                    db.cloudsync_interrupt_sync();
                                }
                            }
                        }
                        match cancellation {
                            CloudsyncRecoveryCancellation::Activity => {
                                if matches!(drained_step, Ok(CloudsyncRecoveryStep::Complete)) {
                                    drained_step
                                } else {
                                    Ok(CloudsyncRecoveryStep::Deferred)
                                }
                            }
                            CloudsyncRecoveryCancellation::Shutdown
                            | CloudsyncRecoveryCancellation::AuthChanged => return,
                        }
                    }
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
                            CloudsyncRecoveryStep::Deferred => {
                                continue;
                            }
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

        for table in anlg_db_app::cloudsync_table_registry()
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
    ) -> Option<(String, anlg_db_core::CloudsyncAuth)> {
        self.cloudsync_full_resync_task
            .lock()
            .await
            .as_ref()
            .map(|task| (task.generation.clone(), task.config.auth.clone()))
    }

    pub async fn start_cloudsync(&self) -> Result<()> {
        let _control_operation = self.cloudsync_control_guard().await?;
        self.ensure_legacy_migration_verified().await?;
        self.e2ee_sync_hook.request_reconciliation();
        self.db.cloudsync_start().await?;
        Ok(())
    }

    pub async fn stop_cloudsync(&self) -> Result<()> {
        self.invalidate_cloudsync_auth_generation();
        self.cancel_cloudsync_full_resync().await;
        let _control_operation = self.cloudsync_control_operation.lock().await;
        self.cancel_cloudsync_full_resync().await;
        self.db.cloudsync_stop().await?;
        Ok(())
    }

    pub async fn suspend_cloudsync(&self) -> Result<()> {
        self.invalidate_cloudsync_auth_generation();
        self.cancel_cloudsync_full_resync().await;
        let _control_operation = self.cloudsync_control_operation.lock().await;
        self.cancel_cloudsync_full_resync().await;
        self.db.cloudsync_suspend().await?;
        self.e2ee_sync_hook.clear();
        Ok(())
    }

    pub async fn suspend_cloudsync_for_sign_out(&self) -> Result<()> {
        match self.suspend_cloudsync().await {
            Ok(()) => Ok(()),
            Err(crate::Error::Cloudsync(anlg_db_core::CloudsyncRuntimeError::LocalStatusBusy)) => {
                tracing::warn!(
                    "CloudSync pool teardown remains pending after account sign-out suspension"
                );
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    pub async fn suspend_cloudsync_after_auth_loss(&self) -> Result<()> {
        self.suspend_cloudsync().await
    }

    pub async fn cloudsync_status(&self) -> Result<serde_json::Value> {
        let mut status = serde_json::to_value(self.db.cloudsync_status().await?)?;
        let activity_paused = self.e2ee_sync_hook.activity_paused();
        let deferred_for_capture = self.e2ee_sync_hook.has_activity(CLOUDSYNC_CAPTURE_ACTIVITY);
        {
            let status_object = status.as_object_mut().ok_or_else(|| {
                std::io::Error::other("CloudSync status did not serialize to an object")
            })?;
            status_object.insert(
                "activity_paused".to_string(),
                serde_json::Value::Bool(activity_paused),
            );
            status_object.insert(
                "deferred_for_capture".to_string(),
                serde_json::Value::Bool(deferred_for_capture),
            );
            if activity_paused {
                let recovery_pending = self
                    .scheduled_cloudsync_full_resync
                    .lock()
                    .unwrap()
                    .generation
                    .is_some();
                status_object.insert(
                    "recovery_pending".to_string(),
                    serde_json::Value::Bool(recovery_pending),
                );
                status_object.insert(
                    "recovery_delayed".to_string(),
                    serde_json::Value::Bool(false),
                );
                status_object.insert("recovery_phase".to_string(), serde_json::Value::Null);
                return Ok(status);
            }
        }

        // Read the canonical dirty queue before the native outbox so a concurrent
        // promotion is visible in at least one status snapshot.
        let Ok(Ok(mut connection)) =
            tokio::time::timeout(CLOUDSYNC_STATUS_POOL_RETURN_GRACE, self.db.pool().acquire())
                .await
        else {
            return Ok(status);
        };
        let keys = self.e2ee_sync_hook.snapshot();
        let enrichment = async {
            let local_e2ee_work_pending =
                has_pending_e2ee_dirty_rows_for_status(&mut connection, &keys)
                    .await
                    .map_err(|error| {
                        std::io::Error::other(format!(
                            "failed to inspect pending E2EE replica changes: {error}"
                        ))
                    })?;
            let recovery = anlg_db_app::cloudsync_recovery_state(&mut *connection).await?;
            Ok::<_, crate::Error>((local_e2ee_work_pending, recovery))
        }
        .await;
        connection.return_to_pool().await;
        let (local_e2ee_work_pending, recovery) = enrichment?;
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
        self.invalidate_cloudsync_auth_generation();
        self.cancel_cloudsync_full_resync().await;
        let _control_operation = self.cloudsync_control_operation.lock().await;
        self.cancel_cloudsync_full_resync().await;
        let _write_guard = self.synced_write_barrier.write().await;
        self.db.cloudsync_logout(discard_unsent_changes).await?;
        self.e2ee_sync_hook.clear();
        self.e2ee_sync_hook.clear_activities();
        Ok(())
    }
}

async fn has_pending_e2ee_dirty_rows_for_status(
    connection: &mut sqlx::SqliteConnection,
    keys: &HashMap<String, anlg_e2ee::WorkspaceKey>,
) -> std::result::Result<bool, sqlx::Error> {
    if keys.is_empty() {
        return Ok(false);
    }

    let mut workspace_ids = keys.keys().collect::<Vec<_>>();
    workspace_ids.sort_unstable();
    let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT EXISTS (
           SELECT 1
           FROM e2ee_dirty_rows AS dirty
             INDEXED BY sqlite_autoindex_e2ee_dirty_rows_1
           WHERE dirty.workspace_id IN (",
    );
    let mut separated = query.separated(", ");
    for workspace_id in workspace_ids {
        separated.push_bind(workspace_id);
    }
    separated.push_unseparated(
        ")
           AND NOT (
             dirty.table_name = 'transcripts'
             AND EXISTS (
               SELECT 1
               FROM (
                 SELECT
                   id,
                   CASE WHEN json_valid(value_json) THEN value_json ELSE '{}' END AS marker_json
                 FROM app_settings
                   INDEXED BY sqlite_autoindex_app_settings_1
                 WHERE id >= 'capture_lifecycle_pending:'
                   AND id < 'capture_lifecycle_pending;'
               ) AS capture
               WHERE
                 length(capture.id) > length('capture_lifecycle_pending:')
                 AND json_type(capture.marker_json, '$.version') IN ('integer', 'real')
                 AND json_extract(capture.marker_json, '$.version') = 1
                 AND CASE json_extract(capture.marker_json, '$.phase')
                   WHEN 'capturing' THEN 1
                   WHEN 'finalizing' THEN 0
                   ELSE CASE
                     WHEN json_extract(capture.marker_json, '$.summaryMode')
                       IN ('regenerate', 'if_empty')
                       THEN 0
                     ELSE 1
                   END
                 END = 1
                 AND json_type(capture.marker_json, '$.sessionId') = 'text'
                 AND json_extract(capture.marker_json, '$.sessionId')
                   = substr(capture.id, length('capture_lifecycle_pending:') + 1)
                 AND json_type(capture.marker_json, '$.transcriptId') = 'text'
                 AND json_extract(capture.marker_json, '$.transcriptId') != ''
                 AND json_type(capture.marker_json, '$.startedAt') IN ('integer', 'real')
                 AND abs(json_extract(capture.marker_json, '$.startedAt'))
                   <= 1.7976931348623157e308
                 AND json_type(capture.marker_json, '$.createdAt') = 'text'
                 AND json_type(capture.marker_json, '$.audioOffsetMs') IN ('integer', 'real')
                 AND abs(json_extract(capture.marker_json, '$.audioOffsetMs'))
                   <= 1.7976931348623157e308
                 AND json_type(capture.marker_json, '$.preserveExistingTranscript')
                   IN ('true', 'false')
                 AND json_type(capture.marker_json, '$.ownerUserId') = 'text'
                 AND json_type(capture.marker_json, '$.memo') = 'text'
                 AND json_extract(capture.marker_json, '$.transcriptId') = dirty.row_id
               LIMIT 1
             )
           )
           LIMIT 1
         )",
    );
    query.build_query_scalar().fetch_one(&mut *connection).await
}

async fn wait_until_cloudsync_auth_generation_changes(
    generation: &std::sync::atomic::AtomicU64,
    changed: &tokio::sync::Notify,
    expected: u64,
) {
    loop {
        let generation_changed = changed.notified();
        tokio::pin!(generation_changed);
        generation_changed.as_mut().enable();
        if generation.load(std::sync::atomic::Ordering::Acquire) != expected {
            return;
        }
        generation_changed.await;
    }
}

fn normalized_cloudsync_activity_part(value: String, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(
            std::io::Error::other(format!("CloudSync activity {label} cannot be empty")).into(),
        );
    }
    let max_len = if label == "activity" { 32 } else { 128 };
    if value.len() > max_len || value.chars().any(char::is_control) {
        return Err(std::io::Error::other(format!("CloudSync activity {label} is invalid")).into());
    }
    Ok(value.to_string())
}

impl Drop for PluginDbRuntime {
    fn drop(&mut self) {
        self.scheduled_cloudsync_full_resync
            .lock()
            .unwrap()
            .cancel();
        if let Some(mut task) = self.cloudsync_full_resync_task.get_mut().take()
            && let Some(shutdown_tx) = task.shutdown_tx.take()
        {
            let _ = shutdown_tx.send(());
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

async fn require_disposable_cloudsync_replica(
    db: &Db,
    cancellation: CloudsyncOperationCancellation<'_>,
) -> Result<()> {
    cancellation.check()?;
    let settings_exist: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1
           FROM sqlite_master
           WHERE type = 'table' AND name = 'cloudsync_table_settings'
         )",
    )
    .fetch_one(db.pool())
    .await?;
    cancellation.check()?;
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
    cancellation.check()?;
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
    config: &anlg_db_core::CloudsyncRuntimeConfig,
    cancellation: CloudsyncOperationCancellation<'_>,
) -> Result<()> {
    cancellation.check()?;
    db.cloudsync_prepare_manual_transport(config.clone())
        .await?;
    cancellation.check()?;
    if !anlg_db_app::cloudsync_encrypted_replica_is_empty(db.pool()).await? {
        return Err(std::io::Error::other(
            "CloudSync recovery refused to reinstall a filter on a populated replica",
        )
        .into());
    }
    cancellation.check()?;
    db.cloudsync_set_filter(CLOUDSYNC_REPLICA_TABLE, CLOUDSYNC_WRITE_FILTER)
        .await
        .map_err(anlg_db_core::CloudsyncRuntimeError::from)?;
    cancellation.check()?;
    anlg_db_app::mark_cloudsync_write_filter_installed(db.pool()).await?;
    cancellation.check()?;
    require_disposable_cloudsync_replica(db, cancellation).await
}

async fn prepare_cloudsync_poison_recovery(
    db: &Db,
    account_user_id: &str,
    workspace_id: &str,
    key: &anlg_e2ee::WorkspaceKey,
    cancellation: CloudsyncOperationCancellation<'_>,
) -> Result<String> {
    cancellation.check()?;
    require_disposable_cloudsync_replica(db, cancellation).await?;
    cancellation.check()?;
    let generation =
        anlg_db_app::stage_cloudsync_poison_recovery(db.pool(), account_user_id, workspace_id)
            .await?;
    cancellation.check()?;
    anlg_db_app::ensure_cloudsync_recovery_state(
        db.pool(),
        &generation,
        account_user_id,
        workspace_id,
        key,
    )
    .await?;
    cancellation.check()?;
    Ok(generation)
}

async fn discard_cloudsync_recovery_replica(
    db: &Db,
    config: &anlg_db_core::CloudsyncRuntimeConfig,
    generation: &str,
    cancellation: CloudsyncOperationCancellation<'_>,
) -> Result<()> {
    cancellation.check()?;
    require_disposable_cloudsync_replica(db, cancellation).await?;
    cancellation.check()?;
    db.cloudsync_logout(true).await?;
    cancellation.check()?;
    prepare_manual_cloudsync_recovery_transport(db, config, cancellation).await?;
    cancellation.check()?;
    if !anlg_db_app::cloudsync_encrypted_replica_is_empty(db.pool()).await? {
        return Err(std::io::Error::other(
            "CloudSync replica was not empty after the first recovery logout",
        )
        .into());
    }
    cancellation.check()?;
    if !anlg_db_app::advance_cloudsync_recovery_phase(
        db.pool(),
        generation,
        anlg_db_app::CloudsyncRecoveryPhase::NeedFirstLogout,
        anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert,
    )
    .await?
    {
        return Err(anlg_db_app::CloudsyncWorkspaceError::RecoveryConflict.into());
    }
    cancellation.check()?;
    Ok(())
}

async fn flush_manual_cloudsync_pending(
    db: &Db,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<CloudsyncPendingFlush> {
    let batch = db.cloudsync_manual_pending_payload_batch().await?;
    if !batch.complete || !batch.fits {
        return Err(std::io::Error::other(format!(
            "CloudSync pending payload is not safely bounded ({} chunks, {} bytes)",
            batch.chunks, batch.bytes
        ))
        .into());
    }
    if batch.chunks == 0 {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(CloudsyncPendingFlush::Waiting);
        }
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

    if cancelled.load(std::sync::atomic::Ordering::Acquire) {
        return Ok(CloudsyncPendingFlush::Waiting);
    }
    if let Ok(status) = db.cloudsync_manual_network_status().await
        && db
            .cloudsync_manual_reconcile_confirmed_pending_payload(batch, &status)
            .await?
    {
        return Ok(CloudsyncPendingFlush::Progressed);
    }

    if cancelled.load(std::sync::atomic::Ordering::Acquire) {
        return Ok(CloudsyncPendingFlush::Waiting);
    }
    match db.cloudsync_manual_send_only(cancelled).await {
        Ok(result) if cloudsync_send_completed(&result) => Ok(CloudsyncPendingFlush::Progressed),
        Ok(result) if cloudsync_send_made_progress(&result) => {
            Ok(CloudsyncPendingFlush::Progressed)
        }
        Ok(_) => Ok(CloudsyncPendingFlush::Waiting),
        Err(send_error) => {
            if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                return Err(send_error.into());
            }
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

fn cloudsync_send_completed(result: &anlg_db_core::CloudsyncNetworkResult) -> bool {
    let Some(send) = result.send.as_ref() else {
        return false;
    };
    send.status.eq_ignore_ascii_case("synced") && send.last_failure.is_none()
}

fn cloudsync_send_made_progress(result: &anlg_db_core::CloudsyncNetworkResult) -> bool {
    result
        .send
        .as_ref()
        .is_some_and(|send| send.chunks > 0 && send.last_failure.is_none())
}

fn cloudsync_receive_completed(result: &anlg_db_core::CloudsyncNetworkResult) -> bool {
    result.receive.as_ref().is_some_and(|receive| {
        receive.complete && receive.error.is_none() && receive.last_failure.is_none()
    })
}

fn cloudsync_receive_delivered(result: &anlg_db_core::CloudsyncNetworkResult) -> bool {
    result.receive.as_ref().is_some_and(|receive| {
        receive.chunks > 0 && receive.error.is_none() && receive.last_failure.is_none()
    })
}

fn cloudsync_receive_incomplete(result: &anlg_db_core::CloudsyncNetworkResult) -> bool {
    result.receive.as_ref().is_some_and(|receive| {
        !receive.complete && receive.error.is_none() && receive.last_failure.is_none()
    })
}

fn cloudsync_receive_requires_reconciliation(
    result: &anlg_db_core::CloudsyncNetworkResult,
) -> bool {
    result
        .receive
        .as_ref()
        .is_some_and(|receive| receive.chunks > 0)
        || cloudsync_receive_incomplete(result)
}

fn cloudsync_receive_delivered_final(result: &anlg_db_core::CloudsyncNetworkResult) -> bool {
    cloudsync_receive_delivered(result)
        && result
            .receive
            .as_ref()
            .is_some_and(|receive| receive.complete)
}

fn cloudsync_recovery_snapshot_ready(
    barrier_is_exact: bool,
    result: &anlg_db_core::CloudsyncNetworkResult,
) -> bool {
    barrier_is_exact && cloudsync_receive_delivered_final(result)
}

fn is_permanent_cloudsync_workspace_rejection(
    error: &anlg_db_app::CloudsyncWorkspaceError,
) -> bool {
    matches!(
        error,
        anlg_db_app::CloudsyncWorkspaceError::InvalidWorkspaceId
            | anlg_db_app::CloudsyncWorkspaceError::InvalidBinding
            | anlg_db_app::CloudsyncWorkspaceError::AccountMismatch
            | anlg_db_app::CloudsyncWorkspaceError::ForeignWorkspace { .. }
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

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
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
const CLOUDSYNC_STATUS_POOL_RETURN_GRACE: std::time::Duration =
    std::time::Duration::from_millis(10);
const CLOUDSYNC_ACTIVITY_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
const CLOUDSYNC_REPLICA_TABLE: &str = "e2ee_records";
const E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT: i64 = 64;
const E2EE_CLOUDSYNC_WITNESS_REPAIR_BYTE_LIMIT: usize = 16 * 1024 * 1024;
const CLOUDSYNC_WRITE_FILTER: &str =
    "workspace_id IN (SELECT allowed_workspace_id FROM cloudsync_writable_workspaces)";
const CLOUDSYNC_CAPTURE_ACTIVITY: &str = "capture";

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct CloudsyncActivity {
    activity: String,
    key: String,
}

pub(crate) struct CloudsyncTokenConfiguration {
    database_id: String,
    token: String,
    account_user_id: String,
    workspace_projection: Option<hypr_db_app::CloudsyncWorkspaceProjection>,
    e2ee_witness: crate::CloudsyncE2eeWitness,
}

impl CloudsyncTokenConfiguration {
    pub(crate) fn new(
        database_id: String,
        token: String,
        account_user_id: String,
        workspace_projection: Option<hypr_db_app::CloudsyncWorkspaceProjection>,
        e2ee_witness: crate::CloudsyncE2eeWitness,
    ) -> Self {
        Self {
            database_id,
            token,
            account_user_id,
            workspace_projection,
            e2ee_witness,
        }
    }
}

#[derive(Default)]
struct E2eeSyncHook {
    keys: std::sync::RwLock<HashMap<String, hypr_e2ee::WorkspaceKey>>,
    witness: std::sync::RwLock<Option<crate::e2ee_witness::E2eeWitnessClient>>,
    activities: std::sync::RwLock<HashSet<CloudsyncActivity>>,
    activity_changed: tokio::sync::Notify,
    reconciliation_requested_epoch: std::sync::atomic::AtomicU64,
    reconciliation_completed_epoch: std::sync::atomic::AtomicU64,
    active_sync_generation: std::sync::atomic::AtomicU64,
    active_sync_cancellation:
        std::sync::Mutex<Option<(u64, crate::e2ee_witness::E2eeWitnessCancellation)>>,
    #[cfg(test)]
    received_apply_cancellation_checks: std::sync::atomic::AtomicU64,
}

struct ActiveE2eeSync<'a> {
    hook: &'a E2eeSyncHook,
    generation: u64,
    cancellation: crate::e2ee_witness::E2eeWitnessCancellation,
}

impl Drop for ActiveE2eeSync<'_> {
    fn drop(&mut self) {
        let mut active = self.hook.active_sync_cancellation.lock().unwrap();
        if active
            .as_ref()
            .is_some_and(|(generation, _)| *generation == self.generation)
        {
            active.take();
        }
    }
}

impl E2eeSyncHook {
    fn set_personal_workspace(
        &self,
        workspace_id: &str,
        recovery_key: &hypr_e2ee::RecoveryKey,
    ) -> std::result::Result<(), hypr_e2ee::Error> {
        let key = recovery_key.workspace_key(workspace_id)?;
        *self.keys.write().unwrap() = HashMap::from([(workspace_id.to_string(), key)]);
        self.request_reconciliation();
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
        let requested = self
            .reconciliation_requested_epoch
            .load(std::sync::atomic::Ordering::Acquire);
        self.reconciliation_completed_epoch
            .fetch_max(requested, std::sync::atomic::Ordering::AcqRel);
    }

    fn clear_activities(&self) {
        let mut activities = self.activities.write().unwrap();
        if activities.is_empty() {
            return;
        }
        activities.clear();
        drop(activities);
        self.activity_changed.notify_waiters();
    }

    fn snapshot(&self) -> HashMap<String, hypr_e2ee::WorkspaceKey> {
        self.keys.read().unwrap().clone()
    }

    fn set_witness(&self, witness: crate::e2ee_witness::E2eeWitnessClient) {
        *self.witness.write().unwrap() = Some(witness);
        self.request_reconciliation();
    }

    fn witness(&self) -> Option<crate::e2ee_witness::E2eeWitnessClient> {
        self.witness.read().unwrap().clone()
    }

    fn begin_activity(&self, activity: String, key: String) {
        self.activities
            .write()
            .unwrap()
            .insert(CloudsyncActivity { activity, key });
        self.activity_changed.notify_waiters();
    }

    fn end_activity(&self, activity: &str, key: &str) -> bool {
        let mut activities = self.activities.write().unwrap();
        let removed = activities.remove(&CloudsyncActivity {
            activity: activity.to_string(),
            key: key.to_string(),
        });
        removed && activities.is_empty()
    }

    fn activity_paused(&self) -> bool {
        !self.activities.read().unwrap().is_empty()
    }

    fn has_activity_lease(&self, activity: &str, key: &str) -> bool {
        self.activities
            .read()
            .unwrap()
            .contains(&CloudsyncActivity {
                activity: activity.to_string(),
                key: key.to_string(),
            })
    }

    fn has_activity(&self, activity: &str) -> bool {
        self.activities
            .read()
            .unwrap()
            .iter()
            .any(|lease| lease.activity == activity)
    }

    async fn wait_until_activity_resumed(&self) {
        loop {
            let resumed = self.activity_changed.notified();
            tokio::pin!(resumed);
            resumed.as_mut().enable();
            if !self.activity_paused() {
                return;
            }
            resumed.await;
        }
    }

    async fn wait_until_activity_paused(&self) {
        loop {
            let paused = self.activity_changed.notified();
            tokio::pin!(paused);
            paused.as_mut().enable();
            if self.activity_paused() {
                return;
            }
            paused.await;
        }
    }

    fn notify_activity_changed(&self) {
        self.activity_changed.notify_waiters();
    }

    fn begin_sync(&self) -> ActiveE2eeSync<'_> {
        let generation = self
            .active_sync_generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
            .wrapping_add(1);
        let cancellation = crate::e2ee_witness::E2eeWitnessCancellation::default();
        *self.active_sync_cancellation.lock().unwrap() = Some((generation, cancellation.clone()));
        ActiveE2eeSync {
            hook: self,
            generation,
            cancellation,
        }
    }

    fn cancel_active_sync(&self) {
        let cancellation = self
            .active_sync_cancellation
            .lock()
            .unwrap()
            .as_ref()
            .map(|(_, cancellation)| cancellation.clone());
        if let Some(cancellation) = cancellation {
            cancellation.cancel();
        }
    }

    fn received_apply_cancelled(
        &self,
        cancellation: &crate::e2ee_witness::E2eeWitnessCancellation,
    ) -> bool {
        #[cfg(test)]
        self.received_apply_cancellation_checks
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        cancellation.is_cancelled()
    }

    fn request_reconciliation(&self) -> u64 {
        self.reconciliation_requested_epoch
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
            .checked_add(1)
            .expect("E2EE reconciliation epoch overflowed")
    }

    fn reconciliation_request_epoch(&self) -> Option<u64> {
        let requested = self
            .reconciliation_requested_epoch
            .load(std::sync::atomic::Ordering::Acquire);
        let completed = self
            .reconciliation_completed_epoch
            .load(std::sync::atomic::Ordering::Acquire);
        (requested > completed).then_some(requested)
    }

    fn complete_reconciliation(&self, epoch: u64) {
        self.reconciliation_completed_epoch
            .fetch_max(epoch, std::sync::atomic::Ordering::AcqRel);
    }

    fn reconciliation_requested(&self) -> bool {
        self.reconciliation_request_epoch().is_some()
    }

    async fn prepare_local_snapshot(
        &self,
        pool: &sqlx::SqlitePool,
        cancellation: &crate::e2ee_witness::E2eeWitnessCancellation,
    ) -> std::result::Result<(), hypr_db_app::E2eeReplicaError> {
        hypr_db_app::encrypt_e2ee_replica_changes_deferring_active_captures_cancellable(
            pool,
            &self.snapshot(),
            || cancellation.is_cancelled(),
        )
        .await
        .map(|_| ())
    }
}

impl hypr_db_core::CloudsyncSyncHook for E2eeSyncHook {
    fn activity_paused(&self) -> bool {
        self.activity_paused()
    }

    fn before_sync<'a>(
        &'a self,
        pool: &'a sqlx::SqlitePool,
    ) -> hypr_db_core::CloudsyncBeforeHookFuture<'a> {
        let keys = self.snapshot();
        let witness = self.witness();
        Box::pin(async move {
            let active_sync = self.begin_sync();
            let cancellation = active_sync.cancellation.clone();
            let operation = async {
                cancellation.check()?;
                let witness = witness.ok_or_else(|| {
                    std::io::Error::other("E2EE freshness witness is not configured")
                })?;
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
                cancellation.check()?;
                if full_resync_pending {
                    tracing::debug!(
                        "skipping outbound E2EE publication while CloudSync hydrates a clean snapshot"
                    );
                    return Ok(hypr_db_core::CloudsyncSyncDirective::ReceiveOnly);
                }
                witness
                    .refresh_notifying_cancellable(
                        pool,
                        key,
                        || {
                            self.request_reconciliation();
                        },
                        &cancellation,
                    )
                    .await?;
                cancellation.check()?;
                let stats =
                    hypr_db_app::encrypt_e2ee_replica_changes_bounded_deferring_active_captures_cancellable(
                        pool,
                        &keys,
                        E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT,
                        || cancellation.is_cancelled(),
                    )
                    .await
                    .map_err(|error| {
                        std::io::Error::other(format!("E2EE pre-sync encryption failed: {error}"))
                    })?;
                cancellation.check()?;
                tracing::debug!(
                    encrypted_fields = stats.encrypted_fields,
                    "prepared encrypted CloudSync replica"
                );
                witness
                    .publish_and_refresh_notifying_cancellable(
                        pool,
                        key,
                        || {
                            self.request_reconciliation();
                        },
                        &cancellation,
                    )
                    .await?;
                cancellation.check()?;
                Ok(hypr_db_core::CloudsyncSyncDirective::SendAndReceive)
            };
            tokio::pin!(operation);
            tokio::select! {
                biased;
                _ = self.wait_until_activity_paused() => {
                    cancellation.cancel();
                    let _ = operation.await;
                    Ok(hypr_db_core::CloudsyncSyncDirective::Deferred)
                }
                result = &mut operation => result,
            }
        })
    }

    fn after_sync<'a>(
        &'a self,
        pool: &'a sqlx::SqlitePool,
        result: &'a hypr_db_core::CloudsyncNetworkResult,
    ) -> hypr_db_core::CloudsyncHookFuture<'a> {
        if cloudsync_receive_requires_reconciliation(result) {
            self.request_reconciliation();
        }
        let keys = self.snapshot();
        let witness = self.witness();
        Box::pin(async move {
            let active_sync = self.begin_sync();
            let cancellation = active_sync.cancellation.clone();
            let operation = async {
                cancellation.check()?;
                let witness = witness.ok_or_else(|| {
                    std::io::Error::other("E2EE freshness witness is not configured")
                })?;
                let key = keys.get(witness.workspace_id()).ok_or_else(|| {
                    std::io::Error::other("E2EE freshness witness identity is not configured")
                })?;
                witness
                    .refresh_notifying_cancellable(
                        pool,
                        key,
                        || {
                            self.request_reconciliation();
                        },
                        &cancellation,
                    )
                    .await?;
                cancellation.check()?;
                let recovery_pending = hypr_db_app::cloudsync_full_resync_generation(pool)
                    .await
                    .map_err(|error| {
                        std::io::Error::other(format!(
                            "failed to inspect CloudSync full resync state: {error}"
                        ))
                    })?
                    .is_some();
                cancellation.check()?;
                let snapshot_complete = !recovery_pending && cloudsync_receive_completed(result);
                let reconciliation_epoch = self.reconciliation_request_epoch();
                let stats = if let Some(reconciliation_epoch) = reconciliation_epoch {
                    let stats =
                        hypr_db_app::apply_received_e2ee_replica_changes_with_witness_cancellable(
                            pool,
                            &keys,
                            snapshot_complete,
                            || self.received_apply_cancelled(&cancellation),
                        )
                        .await
                        .map_err(|error| {
                            std::io::Error::other(format!(
                                "E2EE post-sync decryption failed: {error}"
                            ))
                        })?;
                    cancellation.check()?;
                    if stats.remaining_replica_changes || !snapshot_complete {
                        self.request_reconciliation();
                    }
                    self.complete_reconciliation(reconciliation_epoch);
                    stats
                } else {
                    hypr_db_app::E2eeReplicaStats::default()
                };
                tracing::debug!(
                    applied_fields = stats.applied_fields,
                    skipped_local_changes = stats.skipped_local_changes,
                    rejected_rollbacks = stats.rejected_rollbacks,
                    rejected_unwitnessed = stats.rejected_unwitnessed,
                    snapshot_complete,
                    "applied encrypted CloudSync replica"
                );
                let local_work_remaining = stats.remaining_replica_changes
                    || self.reconciliation_requested()
                    || hypr_db_app::has_pending_e2ee_dirty_rows_deferring_active_captures(
                        pool, &keys,
                    )
                    .await
                    .map_err(|error| {
                        std::io::Error::other(format!(
                            "failed to inspect pending E2EE replica changes: {error}"
                        ))
                    })?;
                cancellation.check()?;
                Ok(hypr_db_core::CloudsyncHookOutcome {
                    local_work_remaining,
                    deferred: false,
                })
            };
            tokio::pin!(operation);
            tokio::select! {
                biased;
                _ = self.wait_until_activity_paused() => {
                    cancellation.cancel();
                    let _ = operation.await;
                    Ok(hypr_db_core::CloudsyncHookOutcome {
                        local_work_remaining: true,
                        deferred: true,
                    })
                }
                result = &mut operation => result,
            }
        })
    }

    fn cancel_active_sync(&self) {
        E2eeSyncHook::cancel_active_sync(self);
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
    cloudsync_control_operation: std::sync::Arc<tokio::sync::Mutex<()>>,
    cloudsync_activity_acquisition: tokio::sync::Mutex<()>,
    cloudsync_auth_generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
    cloudsync_auth_changed: std::sync::Arc<tokio::sync::Notify>,
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
        workspace_projection: Option<hypr_db_app::CloudsyncWorkspaceProjection>,
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
        recovery_key: Option<(String, hypr_e2ee::RecoveryKey)>,
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
                let installed = hypr_db_app::cloudsync_write_filter_installed(
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
                    match hypr_db_app::stage_cloudsync_workspace_reconciliation_cancellable(
                        self.db.pool(),
                        projection,
                        || cancellation.is_cancelled(),
                    )
                    .await
                    {
                        Ok(reconciliation) => reconciliation,
                        Err(hypr_db_app::CloudsyncWorkspaceError::ProjectionCancelled) => {
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
            hypr_db_app::set_cloudsync_personal_write_scope(
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
            match hypr_db_app::commit_cloudsync_workspace_projection_cancellable(
                self.db.pool(),
                projection,
                requires_full_resync,
                || cancellation.is_cancelled(),
            )
            .await
            {
                Ok(_) => {}
                Err(hypr_db_app::CloudsyncWorkspaceError::ProjectionCancelled) => {
                    return Err(crate::Error::CloudsyncConfigurationCancelled);
                }
                Err(error) => return Err(error.into()),
            }
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
        }
        self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;

        if let Some(generation) =
            hypr_db_app::cloudsync_full_resync_generation(self.db.pool()).await?
        {
            self.ensure_cloudsync_configuration_active(auth_generation, cancellation)?;
            hypr_db_app::ensure_cloudsync_recovery_state(
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
            if hypr_db_app::cloudsync_recovery_state(self.db.pool())
                .await?
                .is_some_and(|state| {
                    state.generation == generation
                        && state.phase == hypr_db_app::CloudsyncRecoveryPhase::NeedFirstLogout
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
        let _control_operation = self.cloudsync_control_operation.lock().await;
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

    async fn claim_cloudsync_workspace(
        &self,
        account_user_id: String,
        cancellation: &crate::e2ee_witness::E2eeWitnessCancellation,
    ) -> Result<bool> {
        cancellation.check()?;
        self.ensure_app_schema().await?;
        cancellation.check()?;
        let claimed =
            hypr_db_app::cloudsync_workspace_is_claimed_by(self.db.pool(), &account_user_id).await;
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
        match hypr_db_app::claim_cloudsync_workspace_cancellable(
            self.db.pool(),
            &account_user_id,
            || cancellation.is_cancelled(),
        )
        .await
        {
            Ok(()) => Ok(true),
            Err(hypr_db_app::CloudsyncWorkspaceError::ClaimCancelled) => {
                Err(crate::Error::CloudsyncConfigurationCancelled)
            }
            Err(error) if is_permanent_cloudsync_workspace_rejection(&error) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    async fn prepare_cloudsync_config_fail_closed(
        &self,
        config: hypr_db_core::CloudsyncRuntimeConfig,
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
        for table_name in hypr_db_app::E2EE_DOMAIN_TABLES {
            let enabled = hypr_db_core::cloudsync_is_enabled_on(self.db.pool(), table_name)
                .await
                .map_err(hypr_db_core::CloudsyncRuntimeError::from)?;
            cancellation.check()?;
            if enabled {
                self.db
                    .cloudsync_cleanup(table_name)
                    .await
                    .map_err(hypr_db_core::CloudsyncRuntimeError::from)?;
                cancellation.check()?;
            }
        }
        Ok(())
    }

    async fn prepare_e2ee_cutover_and_initialize_witness(
        &self,
        witness: &crate::e2ee_witness::E2eeWitnessClient,
        key: &hypr_e2ee::WorkspaceKey,
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
                    let _control_operation = control_operation.lock().await;
                    if cloudsync_recovery_cancelled(&recovery_cancelled)
                        || e2ee_sync_hook.activity_paused()
                    {
                        return Ok(CloudsyncRecoveryStep::Deferred);
                    }
                    let state = hypr_db_app::cloudsync_recovery_state(db.pool())
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
                        hypr_db_app::CloudsyncRecoveryPhase::NeedFirstLogout => {
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
                        hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert => {
                            hypr_db_app::insert_cloudsync_recovery_barrier(db.pool(), &state, &key)
                                .await?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if !hypr_db_app::advance_cloudsync_recovery_phase(
                                db.pool(),
                                &generation,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm,
                            )
                            .await?
                            {
                                return Err(
                                    hypr_db_app::CloudsyncWorkspaceError::RecoveryConflict.into()
                                );
                            }
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm => {
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
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if !hypr_db_app::advance_cloudsync_recovery_phase(
                                db.pool(),
                                &generation,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedCleanReceive,
                            )
                            .await?
                            {
                                return Err(
                                    hypr_db_app::CloudsyncWorkspaceError::RecoveryConflict.into()
                                );
                            }
                            clean_receive_attempt_started = false;
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        hypr_db_app::CloudsyncRecoveryPhase::NeedCleanReceive => {
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
                                    .map_err(hypr_db_core::CloudsyncRuntimeError::from)?;
                                if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                    return Ok(CloudsyncRecoveryStep::Deferred);
                                }
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
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            witness
                                .refresh_cancellable(db.pool(), &key, &witness_cancellation)
                                .await?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            let apply = hypr_db_app::apply_received_e2ee_replica_changes_with_witness_cancellable(
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
                                hypr_db_app::cloudsync_recovery_barrier_is_exact(
                                    db.pool(),
                                    &state,
                                    &key,
                                )
                                .await?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
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
                                hypr_db_app::repair_e2ee_replica_from_witness_bounded_cancellable(
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
                            let apply = hypr_db_app::apply_received_e2ee_replica_changes_with_witness_cancellable(
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
                            if hypr_db_app::has_pending_e2ee_witness_repairs_cancellable(
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

                            let local = hypr_db_app::encrypt_e2ee_replica_changes_bounded_deferring_active_captures_cancellable(
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
                            if hypr_db_app::has_pending_e2ee_dirty_rows_deferring_active_captures(
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
                            if hypr_db_app::has_pending_e2ee_witness_repairs_cancellable(
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
                            if !hypr_db_app::advance_cloudsync_recovery_phase(
                                db.pool(),
                                &generation,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedWitnessRepair,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierCleanup,
                            )
                            .await?
                            {
                                return Err(
                                    hypr_db_app::CloudsyncWorkspaceError::RecoveryConflict.into()
                                );
                            }
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierCleanup => {
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
                            if hypr_db_app::cloudsync_recovery_barrier_is_exact(
                                db.pool(),
                                &state,
                                &key,
                            )
                            .await?
                            {
                                if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                    return Ok(CloudsyncRecoveryStep::Deferred);
                                }
                                hypr_db_app::delete_cloudsync_recovery_barrier(
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
                            if !hypr_db_app::advance_cloudsync_recovery_phase(
                                db.pool(),
                                &generation,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierCleanup,
                                hypr_db_app::CloudsyncRecoveryPhase::NeedTransportResume,
                            )
                            .await?
                            {
                                return Err(
                                    hypr_db_app::CloudsyncWorkspaceError::RecoveryConflict.into()
                                );
                            }
                            Ok(CloudsyncRecoveryStep::Progressed)
                        }
                        hypr_db_app::CloudsyncRecoveryPhase::NeedTransportResume => {
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            db.cloudsync_resume_prepared_transport().await?;
                            if cloudsync_recovery_cancelled(&recovery_cancelled) {
                                return Ok(CloudsyncRecoveryStep::Deferred);
                            }
                            if !hypr_db_app::complete_cloudsync_recovery(db.pool(), &generation)
                                .await?
                            {
                                return Err(
                                    hypr_db_app::CloudsyncWorkspaceError::RecoveryConflict.into()
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
        let _activity_acquisition = self.cloudsync_activity_acquisition.lock().await;
        match self.suspend_cloudsync().await {
            Ok(()) => {}
            Err(crate::Error::Cloudsync(hypr_db_core::CloudsyncRuntimeError::LocalStatusBusy)) => {
                tracing::warn!(
                    "CloudSync pool teardown remains pending after account sign-out suspension"
                );
            }
            Err(error) => return Err(error),
        }
        self.e2ee_sync_hook.clear_activities();
        Ok(())
    }

    pub async fn suspend_cloudsync_after_auth_loss(&self) -> Result<()> {
        let _activity_acquisition = self.cloudsync_activity_acquisition.lock().await;
        let result = self.suspend_cloudsync().await;
        self.e2ee_sync_hook.clear_activities();
        result
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
        let mut connection = self.db.pool().try_acquire();
        if connection.is_none() {
            tokio::task::yield_now().await;
            connection = self.db.pool().try_acquire();
        }
        if connection.is_none() {
            tokio::time::sleep(CLOUDSYNC_STATUS_POOL_RETURN_GRACE).await;
            connection = self.db.pool().try_acquire();
        }
        let Some(mut connection) = connection else {
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
            let recovery = hypr_db_app::cloudsync_recovery_state(&mut *connection).await?;
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
    keys: &HashMap<String, hypr_e2ee::WorkspaceKey>,
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
    config: &hypr_db_core::CloudsyncRuntimeConfig,
    cancellation: CloudsyncOperationCancellation<'_>,
) -> Result<()> {
    cancellation.check()?;
    db.cloudsync_prepare_manual_transport(config.clone())
        .await?;
    cancellation.check()?;
    if !hypr_db_app::cloudsync_encrypted_replica_is_empty(db.pool()).await? {
        return Err(std::io::Error::other(
            "CloudSync recovery refused to reinstall a filter on a populated replica",
        )
        .into());
    }
    cancellation.check()?;
    db.cloudsync_set_filter(CLOUDSYNC_REPLICA_TABLE, CLOUDSYNC_WRITE_FILTER)
        .await
        .map_err(hypr_db_core::CloudsyncRuntimeError::from)?;
    cancellation.check()?;
    hypr_db_app::mark_cloudsync_write_filter_installed(db.pool()).await?;
    cancellation.check()?;
    require_disposable_cloudsync_replica(db, cancellation).await
}

async fn prepare_cloudsync_poison_recovery(
    db: &Db,
    account_user_id: &str,
    workspace_id: &str,
    key: &hypr_e2ee::WorkspaceKey,
    cancellation: CloudsyncOperationCancellation<'_>,
) -> Result<String> {
    cancellation.check()?;
    require_disposable_cloudsync_replica(db, cancellation).await?;
    cancellation.check()?;
    let generation =
        hypr_db_app::stage_cloudsync_poison_recovery(db.pool(), account_user_id, workspace_id)
            .await?;
    cancellation.check()?;
    hypr_db_app::ensure_cloudsync_recovery_state(
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
    config: &hypr_db_core::CloudsyncRuntimeConfig,
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
    if !hypr_db_app::cloudsync_encrypted_replica_is_empty(db.pool()).await? {
        return Err(std::io::Error::other(
            "CloudSync replica was not empty after the first recovery logout",
        )
        .into());
    }
    cancellation.check()?;
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

fn cloudsync_receive_incomplete(result: &hypr_db_core::CloudsyncNetworkResult) -> bool {
    result.receive.as_ref().is_some_and(|receive| {
        !receive.complete && receive.error.is_none() && receive.last_failure.is_none()
    })
}

fn cloudsync_receive_requires_reconciliation(
    result: &hypr_db_core::CloudsyncNetworkResult,
) -> bool {
    result
        .receive
        .as_ref()
        .is_some_and(|receive| receive.chunks > 0)
        || cloudsync_receive_incomplete(result)
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

    async fn configured_test_hook() -> (E2eeSyncHook, MockServer) {
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
        (hook, witness_server)
    }

    fn receive_result(chunks: u64, complete: bool) -> hypr_db_core::CloudsyncNetworkResult {
        serde_json::from_value(serde_json::json!({
            "receive": {
                "rows": chunks,
                "tables": [],
                "chunks": chunks,
                "bytes": chunks,
                "complete": complete
            }
        }))
        .unwrap()
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

    #[tokio::test]
    async fn idle_sync_skips_replica_reconciliation_until_new_work_arrives() {
        let db = Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let (hook, _witness_server) = configured_test_hook().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-1', 'workspace-1', 'Local session')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        hypr_db_core::CloudsyncSyncHook::before_sync(&hook, db.pool())
            .await
            .unwrap();
        let complete_idle = receive_result(0, true);
        hypr_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &complete_idle)
            .await
            .unwrap();

        let (record_id, original_payload): (String, String) = sqlx::query_as(
            "SELECT replica.id, replica.payload
             FROM e2ee_records AS replica
             INNER JOIN e2ee_witness_records AS witness
               ON witness.workspace_id = replica.workspace_id
              AND witness.record_id = replica.id
              AND witness.payload = replica.payload
             WHERE replica.workspace_id = 'workspace-1'
             ORDER BY replica.id
             LIMIT 1",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        sqlx::query("UPDATE e2ee_records SET payload = 'corrupt' WHERE id = ?")
            .bind(&record_id)
            .execute(db.pool())
            .await
            .unwrap();

        let idle_outcome =
            hypr_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &complete_idle)
                .await
                .unwrap();
        let idle_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&record_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(idle_payload, "corrupt");
        assert!(!idle_outcome.local_work_remaining);

        let delivered = receive_result(1, true);
        hypr_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &delivered)
            .await
            .unwrap();
        let reconciled_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(record_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(reconciled_payload, original_payload);
    }

    #[tokio::test]
    async fn nonfinal_receive_keeps_reconciliation_pending_for_final_snapshot() {
        let db = Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let (hook, _witness_server) = configured_test_hook().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-1', 'workspace-1', 'Remote session')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        hypr_db_core::CloudsyncSyncHook::before_sync(&hook, db.pool())
            .await
            .unwrap();
        hypr_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &receive_result(0, true))
            .await
            .unwrap();
        let missing_record_id: String = sqlx::query_scalar(
            "SELECT replica.id
             FROM e2ee_records AS replica
             INNER JOIN e2ee_witness_records AS witness
               ON witness.workspace_id = replica.workspace_id
              AND witness.record_id = replica.id
             WHERE replica.workspace_id = 'workspace-1'
             ORDER BY replica.id
             LIMIT 1",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        sqlx::query("DELETE FROM e2ee_records WHERE id = ?")
            .bind(&missing_record_id)
            .execute(db.pool())
            .await
            .unwrap();

        let partial_outcome = hypr_db_core::CloudsyncSyncHook::after_sync(
            &hook,
            db.pool(),
            &receive_result(1, false),
        )
        .await
        .unwrap();
        assert!(partial_outcome.local_work_remaining);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records WHERE id = ?")
                .bind(&missing_record_id)
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );

        let final_outcome =
            hypr_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &receive_result(0, true))
                .await
                .unwrap();
        assert!(!final_outcome.local_work_remaining);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records WHERE id = ?")
                .bind(&missing_record_id)
                .fetch_one(db.pool())
                .await
                .unwrap(),
            1
        );

        sqlx::query("DELETE FROM e2ee_records WHERE id = ?")
            .bind(&missing_record_id)
            .execute(db.pool())
            .await
            .unwrap();
        let failed_after_chunks: hypr_db_core::CloudsyncNetworkResult =
            serde_json::from_value(serde_json::json!({
                "receive": {
                    "rows": 1,
                    "tables": ["e2ee_records"],
                    "chunks": 1,
                    "bytes": 1,
                    "complete": false,
                    "error": "later chunk failed"
                }
            }))
            .unwrap();
        let failed_outcome =
            hypr_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &failed_after_chunks)
                .await
                .unwrap();
        assert!(failed_outcome.local_work_remaining);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records WHERE id = ?")
                .bind(&missing_record_id)
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );

        let retry_outcome =
            hypr_db_core::CloudsyncSyncHook::after_sync(&hook, db.pool(), &receive_result(0, true))
                .await
                .unwrap();
        assert!(!retry_outcome.local_work_remaining);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records WHERE id = ?")
                .bind(missing_record_id)
                .fetch_one(db.pool())
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn activity_drains_large_no_mismatch_preflight_before_local_write() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = std::sync::Arc::new(PluginDbRuntime::new(std::sync::Arc::clone(&db)));
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        runtime
            .set_e2ee_recovery_key("workspace-1", &recovery_key)
            .unwrap();
        let (configured_hook, _witness_server) = configured_test_hook().await;
        runtime
            .e2ee_sync_hook
            .set_witness(configured_hook.witness().unwrap());

        sqlx::query(
            "WITH RECURSIVE rows(id) AS (
               SELECT 1
               UNION ALL
               SELECT id + 1 FROM rows WHERE id < 50000
             )
             INSERT INTO e2ee_records (id, workspace_id, payload)
             SELECT printf('!unwitnessed-%06d', id), 'workspace-1', 'unwitnessed' FROM rows",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_local_state (
               record_id, workspace_id, table_name, row_id, field_name, revision,
               writer_id, value_tag, payload_hash, payload
             )
             SELECT
               id,
               workspace_id,
               'sessions',
               id,
               '$row',
               1,
               'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
               'unchanged',
               'unchanged',
               payload
             FROM e2ee_records
             WHERE id LIKE '!unwitnessed-%'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let key = runtime.workspace_key("workspace-1").unwrap();
        let mut transaction = db.pool().begin().await.unwrap();
        for index in 0..32 {
            let sealed = key
                .seal_field(
                    "workspace-1",
                    "sessions",
                    &format!("received-session-{index:02}"),
                    "$row",
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    1,
                    false,
                    serde_json::json!(true),
                )
                .unwrap();
            sqlx::query(
                "INSERT INTO e2ee_records (id, workspace_id, payload)
                 VALUES (?, 'workspace-1', ?)",
            )
            .bind(&sealed.record_id)
            .bind(&sealed.payload)
            .execute(&mut *transaction)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO e2ee_witness_records (
                   workspace_id, record_id, revision, writer_id, payload_hash, payload, sequence
                 ) VALUES (
                   'workspace-1', ?, 1, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', ?, ?, ?
                 )",
            )
            .bind(&sealed.record_id)
            .bind(hypr_e2ee::payload_hash(&sealed.payload))
            .bind(&sealed.payload)
            .bind(i64::from(index) + 1)
            .execute(&mut *transaction)
            .await
            .unwrap();
        }
        transaction.commit().await.unwrap();

        let reconcile_runtime = std::sync::Arc::clone(&runtime);
        let reconcile_db = std::sync::Arc::clone(&db);
        let reconcile = tokio::spawn(async move {
            hypr_db_core::CloudsyncSyncHook::after_sync(
                reconcile_runtime.e2ee_sync_hook.as_ref(),
                reconcile_db.pool(),
                &receive_result(1, false),
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while runtime
                .e2ee_sync_hook
                .received_apply_cancellation_checks
                .load(Ordering::Acquire)
                < 70
            {
                assert!(
                    !reconcile.is_finished(),
                    "received-replica reconciliation finished before cancellation"
                );
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("received-replica reconciliation did not enter bounded preflight");

        let cancelled_at = std::time::Instant::now();
        runtime
            .e2ee_sync_hook
            .begin_activity("capture".to_string(), "session-1".to_string());
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(2), reconcile)
            .await
            .expect("activity did not drain received-replica reconciliation")
            .unwrap()
            .unwrap();
        assert!(outcome.deferred);
        assert!(outcome.local_work_remaining);
        assert!(cancelled_at.elapsed() < std::time::Duration::from_secs(2));

        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('local-after-received-cancel', 'workspace-1', 'workspace-1', 'Local')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("local write remained blocked after received-replica cancellation")
        .unwrap();
        assert!(runtime.e2ee_sync_hook.end_activity("capture", "session-1"));
        runtime.e2ee_sync_hook.notify_activity_changed();
    }

    #[test]
    fn reconciliation_epochs_preserve_newer_requests() {
        let hook = E2eeSyncHook::default();
        hook.request_reconciliation();
        let first_epoch = hook.reconciliation_request_epoch().unwrap();

        hook.request_reconciliation();
        hook.complete_reconciliation(first_epoch);

        assert!(hook.reconciliation_requested());
        let second_epoch = hook.reconciliation_request_epoch().unwrap();
        assert!(second_epoch > first_epoch);
        hook.complete_reconciliation(second_epoch);
        assert!(!hook.reconciliation_requested());

        hook.request_reconciliation();
        hook.clear();
        assert!(!hook.reconciliation_requested());
        hook.request_reconciliation();
        assert!(hook.reconciliation_requested());
    }

    #[tokio::test]
    async fn witness_refresh_failure_keeps_reconciliation_pending() {
        let db = Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let hook = E2eeSyncHook::default();
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        hook.set_personal_workspace("workspace-1", &recovery_key)
            .unwrap();
        let witness_server = MockServer::start().await;
        Mock::given(path("/sync/e2ee/witness/workspace-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "initialized": true,
                "initializedAt": "2026-07-24T00:00:00Z",
                "headSequence": 1,
                "throughSequence": 1,
                "nextAfterSequence": 1,
                "events": [{
                    "sequence": 1,
                    "recordId": "invalid-record",
                    "payloadHash": "invalid-hash",
                    "payload": "invalid-payload"
                }]
            })))
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
        let setup_epoch = hook.reconciliation_request_epoch().unwrap();
        hook.complete_reconciliation(setup_epoch);

        assert!(
            hypr_db_core::CloudsyncSyncHook::before_sync(&hook, db.pool())
                .await
                .is_err()
        );
        assert!(hook.reconciliation_requested());
    }

    #[tokio::test]
    async fn failed_reconciliation_epoch_stays_pending() {
        let db = Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let (hook, _witness_server) = configured_test_hook().await;
        let setup_epoch = hook.reconciliation_request_epoch().unwrap();
        hook.complete_reconciliation(setup_epoch);
        sqlx::query(
            "INSERT INTO e2ee_witness_records (
               workspace_id,
               record_id,
               revision,
               writer_id,
               payload_hash,
               payload,
               sequence
             )
             VALUES (
               'workspace-1',
               'invalid-record',
               1,
               'invalid-writer',
               'invalid-hash',
               'invalid-payload',
               1
             )",
        )
        .execute(db.pool())
        .await
        .unwrap();

        assert!(
            hypr_db_core::CloudsyncSyncHook::after_sync(
                &hook,
                db.pool(),
                &receive_result(1, true),
            )
            .await
            .is_err()
        );
        assert!(hook.reconciliation_requested());
    }

    #[tokio::test]
    async fn cloudsync_activity_leases_are_idempotent_and_preserved_by_identity_clear() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(db);

        runtime
            .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        runtime
            .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        runtime
            .begin_cloudsync_activity("capture".to_string(), "session-2".to_string())
            .await
            .unwrap();
        runtime.e2ee_sync_hook.clear();

        let status = runtime.cloudsync_status().await.unwrap();
        assert_eq!(status["activity_paused"], true);
        assert_eq!(status["deferred_for_capture"], true);
        assert_eq!(status["has_unsent_changes"], serde_json::Value::Null);

        runtime
            .end_cloudsync_activity("capture".to_string(), "missing".to_string())
            .await
            .unwrap();
        assert!(runtime.e2ee_sync_hook.activity_paused());

        runtime
            .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        assert!(runtime.e2ee_sync_hook.activity_paused());
        runtime
            .end_cloudsync_activity("capture".to_string(), "session-2".to_string())
            .await
            .unwrap();
        runtime
            .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        assert!(!runtime.e2ee_sync_hook.activity_paused());
    }

    #[tokio::test]
    async fn cloudsync_activity_pause_is_process_scoped() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
        runtime
            .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        assert!(runtime.e2ee_sync_hook.activity_paused());
        drop(runtime);

        let restarted_runtime = PluginDbRuntime::new(db);
        let status = restarted_runtime.cloudsync_status().await.unwrap();
        assert_eq!(status["activity_paused"], false);
        assert_eq!(status["deferred_for_capture"], false);
    }

    #[tokio::test]
    async fn activity_resume_wait_cannot_lose_its_notification() {
        let hook = E2eeSyncHook::default();
        hook.begin_activity("capture".to_string(), "session-1".to_string());
        let mut resumed = Box::pin(hook.wait_until_activity_resumed());

        tokio::select! {
            biased;
            () = &mut resumed => panic!("activity resume wait completed while paused"),
            _ = tokio::task::yield_now() => {}
        }
        assert!(hook.end_activity("capture", "session-1"));
        hook.notify_activity_changed();
        tokio::time::timeout(std::time::Duration::from_millis(100), resumed)
            .await
            .expect("activity resume notification was lost");

        hook.wait_until_activity_resumed().await;
    }

    #[tokio::test]
    async fn clearing_activities_wakes_resume_waiters() {
        let hook = E2eeSyncHook::default();
        hook.begin_activity("capture".to_string(), "session-1".to_string());
        hook.begin_activity("chat".to_string(), "chat-1".to_string());
        let mut resumed = Box::pin(hook.wait_until_activity_resumed());

        tokio::select! {
            biased;
            () = &mut resumed => panic!("activity resume wait completed while paused"),
            _ = tokio::task::yield_now() => {}
        }
        hook.clear_activities();
        tokio::time::timeout(std::time::Duration::from_millis(100), resumed)
            .await
            .expect("activity clear notification was lost");
        assert!(!hook.activity_paused());
    }

    #[tokio::test]
    async fn paused_cloudsync_status_does_not_probe_the_busy_database() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
        runtime
            .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        let _held_connection = db.pool().acquire().await.unwrap();

        let status = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            runtime.cloudsync_status(),
        )
        .await
        .expect("paused CloudSync status queried the busy database")
        .unwrap();

        assert_eq!(status["activity_paused"], true);
        assert_eq!(status["deferred_for_capture"], true);
    }

    #[tokio::test]
    async fn activity_begin_waits_for_an_in_flight_cloudsync_control_operation() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(db);
        let control_operation = runtime.cloudsync_control_operation.lock().await;
        let mut begin = Box::pin(
            runtime.begin_cloudsync_activity("capture".to_string(), "session-1".to_string()),
        );

        tokio::select! {
            biased;
            result = &mut begin => panic!("activity begin bypassed recovery: {result:?}"),
            _ = tokio::task::yield_now() => {}
        }
        assert!(runtime.e2ee_sync_hook.activity_paused());

        drop(control_operation);
        tokio::time::timeout(std::time::Duration::from_millis(100), begin)
            .await
            .expect("activity begin did not finish after CloudSync control became idle")
            .unwrap();
    }

    #[tokio::test]
    async fn activity_begin_timeout_fails_closed_and_releases_the_new_lease() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(db);
        let control_operation = runtime.cloudsync_control_operation.lock().await;

        let error = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            runtime.begin_cloudsync_activity_with_timeout(
                "capture".to_string(),
                "session-1".to_string(),
                std::time::Duration::from_millis(10),
            ),
        )
        .await
        .expect("activity begin exceeded its drain deadline")
        .unwrap_err();

        assert!(matches!(error, crate::Error::CloudsyncActivityDrainTimeout));
        assert!(!runtime.e2ee_sync_hook.activity_paused());
        assert!(
            !runtime
                .e2ee_sync_hook
                .has_activity_lease("capture", "session-1")
        );
        drop(control_operation);
    }

    #[tokio::test]
    async fn duplicate_activity_begin_timeout_preserves_the_existing_lease() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(db);
        runtime
            .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        let control_operation = runtime.cloudsync_control_operation.lock().await;

        runtime
            .begin_cloudsync_activity_with_timeout(
                "capture".to_string(),
                "session-1".to_string(),
                std::time::Duration::from_millis(10),
            )
            .await
            .unwrap();

        assert!(runtime.e2ee_sync_hook.activity_paused());
        assert!(
            runtime
                .e2ee_sync_hook
                .has_activity_lease("capture", "session-1")
        );
        drop(control_operation);
        runtime
            .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn activity_end_cancels_a_begin_waiting_for_cloudsync_control() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(db);
        let control_operation = runtime.cloudsync_control_operation.lock().await;
        let mut begin = Box::pin(
            runtime.begin_cloudsync_activity("capture".to_string(), "session-1".to_string()),
        );

        tokio::select! {
            biased;
            result = &mut begin => panic!("activity begin bypassed CloudSync control: {result:?}"),
            _ = tokio::task::yield_now() => {}
        }
        runtime
            .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        drop(control_operation);

        let error = tokio::time::timeout(std::time::Duration::from_millis(100), begin)
            .await
            .expect("cancelled activity begin did not finish")
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("ended before synchronization became idle")
        );
    }

    #[tokio::test]
    async fn cloudsync_configuration_and_start_fail_promptly_during_activity() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(db);
        runtime
            .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        let config = serde_json::to_string(&hypr_db_core::CloudsyncRuntimeConfig {
            connection_string: "managed-database-id".to_string(),
            auth: hypr_db_core::CloudsyncAuth::None,
            tables: Vec::new(),
            sync_interval_ms: DEFAULT_CLOUDSYNC_INTERVAL_MS,
            wait_ms: Some(5_000),
            max_retries: Some(3),
        })
        .unwrap();

        let configure_error = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            runtime.configure_cloudsync(config),
        )
        .await
        .expect("CloudSync configuration waited for the activity to finish")
        .unwrap_err();
        assert!(matches!(
            configure_error,
            crate::Error::CloudsyncActivityDeferred
        ));
        assert_eq!(configure_error.to_string(), "cloudsync_activity_deferred");

        let token_error = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            runtime.configure_cloudsync_token(
                "managed-database-id".to_string(),
                "token".to_string(),
                "user-a".to_string(),
                crate::CloudsyncE2eeWitness {
                    endpoint: "https://example.com/witness".to_string(),
                    access_token: "access-token".to_string(),
                },
            ),
        )
        .await
        .expect("CloudSync token configuration waited for the activity to finish")
        .unwrap_err();
        assert!(matches!(
            token_error,
            crate::Error::CloudsyncActivityDeferred
        ));

        let start_error = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            runtime.start_cloudsync(),
        )
        .await
        .expect("CloudSync start waited for the activity to finish")
        .unwrap_err();
        assert!(matches!(
            start_error,
            crate::Error::CloudsyncActivityDeferred
        ));
    }

    #[tokio::test]
    async fn cloudsync_start_prearms_reconciliation() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        sqlx::query(
            "UPDATE storage_migration_state
             SET importer_version = ?, parity_verified = 1
             WHERE id = 'legacy_v1'",
        )
        .bind(hypr_db_app::LEGACY_IMPORTER_VERSION)
        .execute(db.pool())
        .await
        .unwrap();
        let runtime = PluginDbRuntime::new(db);
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        runtime
            .set_e2ee_recovery_key("workspace-1", &recovery_key)
            .unwrap();
        let setup_epoch = runtime
            .e2ee_sync_hook
            .reconciliation_request_epoch()
            .unwrap();
        runtime.e2ee_sync_hook.complete_reconciliation(setup_epoch);

        let _ = runtime.start_cloudsync().await;

        assert!(runtime.e2ee_sync_hook.reconciliation_requested());
    }

    #[tokio::test]
    async fn cloudsync_control_rechecks_activity_after_acquiring_serialization() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(db);
        let control_operation = runtime.cloudsync_control_operation.lock().await;
        let mut start = Box::pin(runtime.start_cloudsync());

        tokio::select! {
            biased;
            result = &mut start => panic!("CloudSync start bypassed control serialization: {result:?}"),
            _ = tokio::task::yield_now() => {}
        }
        runtime
            .e2ee_sync_hook
            .begin_activity("capture".to_string(), "session-1".to_string());

        let error = tokio::time::timeout(std::time::Duration::from_millis(100), start)
            .await
            .expect("CloudSync start remained queued after activity began")
            .unwrap_err();
        assert!(matches!(error, crate::Error::CloudsyncActivityDeferred));
        drop(control_operation);
    }

    #[tokio::test]
    async fn local_account_binding_is_not_deferred_by_cloudsync_activity() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(db);
        runtime
            .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();

        let bound = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            runtime.bind_cloudsync_account("user-a".to_string()),
        )
        .await
        .expect("local account binding waited for the activity to finish")
        .unwrap();
        assert!(bound);
        assert!(
            runtime
                .e2ee_sync_hook
                .has_activity_lease("capture", "session-1")
        );
    }

    #[tokio::test]
    async fn stop_suspend_and_logout_are_not_deferred_by_cloudsync_activity() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(db);
        runtime
            .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();

        let stop = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            runtime.stop_cloudsync(),
        )
        .await
        .expect("CloudSync stop waited for the activity to finish");
        assert!(!matches!(
            stop,
            Err(crate::Error::CloudsyncActivityDeferred)
        ));

        let suspend = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            runtime.suspend_cloudsync(),
        )
        .await
        .expect("CloudSync suspension waited for the activity to finish");
        assert!(!matches!(
            suspend,
            Err(crate::Error::CloudsyncActivityDeferred)
        ));
        assert!(runtime.e2ee_sync_hook.activity_paused());

        let logout = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            runtime.logout_cloudsync(false),
        )
        .await
        .expect("CloudSync logout waited for the activity to finish");
        assert!(!matches!(
            logout,
            Err(crate::Error::CloudsyncActivityDeferred)
        ));
        assert!(!runtime.e2ee_sync_hook.activity_paused());
    }

    #[tokio::test]
    async fn sign_out_suspend_clears_activity_leases() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(db);
        runtime
            .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        runtime
            .begin_cloudsync_activity("chat".to_string(), "chat-1".to_string())
            .await
            .unwrap();

        runtime.suspend_cloudsync_for_sign_out().await.unwrap();

        assert!(!runtime.e2ee_sync_hook.activity_paused());
    }

    #[tokio::test]
    async fn auth_loss_suspend_clears_activity_leases_after_non_busy_error() {
        let db = std::sync::Arc::new(Db::connect_memory().await.unwrap());
        let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
        runtime
            .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        let mut resumed = Box::pin(runtime.e2ee_sync_hook.wait_until_activity_resumed());
        tokio::select! {
            biased;
            () = &mut resumed => panic!("activity resume wait completed while paused"),
            _ = tokio::task::yield_now() => {}
        }
        db.pool().close().await;

        let error = runtime
            .suspend_cloudsync_after_auth_loss()
            .await
            .unwrap_err();

        assert!(!matches!(
            error,
            crate::Error::Cloudsync(hypr_db_core::CloudsyncRuntimeError::LocalStatusBusy)
        ));
        assert!(!runtime.e2ee_sync_hook.activity_paused());
        tokio::time::timeout(std::time::Duration::from_millis(100), resumed)
            .await
            .expect("auth-loss cleanup did not notify activity resume waiters");
    }

    #[tokio::test]
    async fn final_activity_release_resets_the_recovery_delay_clock() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(db);
        runtime
            .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        {
            let mut schedule = runtime.scheduled_cloudsync_full_resync.lock().unwrap();
            schedule.claim("generation-1");
            schedule.last_progress_at =
                Some(std::time::Instant::now() - std::time::Duration::from_secs(61));
        }

        runtime
            .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();

        let schedule = runtime.scheduled_cloudsync_full_resync.lock().unwrap();
        assert!(!schedule.recovery_delayed);
        assert!(!schedule.is_delayed("generation-1"));
    }

    #[tokio::test]
    async fn activity_release_does_not_hide_a_recovery_failure() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(db);
        runtime
            .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        {
            let mut schedule = runtime.scheduled_cloudsync_full_resync.lock().unwrap();
            schedule.claim("generation-1");
            schedule.mark_failure("generation-1");
        }

        runtime
            .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();

        let schedule = runtime.scheduled_cloudsync_full_resync.lock().unwrap();
        assert!(schedule.recovery_delayed);
        assert!(schedule.is_delayed("generation-1"));
    }

    #[test]
    fn cloudsync_activity_identifiers_are_bounded() {
        assert!(normalized_cloudsync_activity_part(" ".to_string(), "activity").is_err());
        assert!(
            normalized_cloudsync_activity_part("capture\n".to_string(), "activity").is_ok(),
            "outer whitespace is intentionally trimmed"
        );
        assert!(normalized_cloudsync_activity_part("cap\nture".to_string(), "activity").is_err());
        assert!(normalized_cloudsync_activity_part("a".repeat(33), "activity").is_err());
        assert!(normalized_cloudsync_activity_part("k".repeat(129), "key").is_err());
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
        let failed_after_chunks: hypr_db_core::CloudsyncNetworkResult =
            serde_json::from_value(serde_json::json!({
                "receive": {
                    "rows": 1,
                    "tables": ["e2ee_records"],
                    "chunks": 1,
                    "complete": false,
                    "error": "later chunk failed"
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
        assert!(!cloudsync_receive_delivered(&failed_after_chunks));
        assert!(cloudsync_receive_requires_reconciliation(
            &failed_after_chunks
        ));
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

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn activity_drains_stalled_full_resync_before_immediate_local_write() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (stalled_tx, stalled_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let server = std::thread::spawn(move || {
            let (mut status_stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            status_stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let size = status_stream.read(&mut request).unwrap();
            assert!(
                String::from_utf8_lossy(&request[..size]).contains("/status"),
                "recovery did not probe status before sending"
            );
            let body = r#"{"lastOptimisticVersion":0,"lastConfirmedVersion":0,"gaps":[]}"#;
            write!(
                status_stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
            status_stream.flush().unwrap();
            drop(status_stream);

            let (stalled_stream, _) = listener.accept().unwrap();
            stalled_tx.send(()).unwrap();
            let _stalled_stream = stalled_stream;
            let _ = release_rx.recv_timeout(std::time::Duration::from_secs(5));
        });

        let db = std::sync::Arc::new(Db::connect_memory().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        hypr_db_app::claim_cloudsync_workspace(db.pool(), "workspace-1")
            .await
            .unwrap();
        db.cloudsync_init(CLOUDSYNC_REPLICA_TABLE, None, None)
            .await
            .unwrap();
        let runtime = std::sync::Arc::new(PluginDbRuntime::new(std::sync::Arc::clone(&db)));
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        runtime
            .set_e2ee_recovery_key("workspace-1", &recovery_key)
            .unwrap();
        let key = runtime.workspace_key("workspace-1").unwrap();
        let (configured_hook, _witness_server) = configured_test_hook().await;
        runtime
            .e2ee_sync_hook
            .set_witness(configured_hook.witness().unwrap());

        let generation =
            hypr_db_app::stage_cloudsync_poison_recovery(db.pool(), "workspace-1", "workspace-1")
                .await
                .unwrap();
        let state = hypr_db_app::ensure_cloudsync_recovery_state(
            db.pool(),
            &generation,
            "workspace-1",
            "workspace-1",
            &key,
        )
        .await
        .unwrap();
        assert!(
            hypr_db_app::advance_cloudsync_recovery_phase(
                db.pool(),
                &generation,
                hypr_db_app::CloudsyncRecoveryPhase::NeedFirstLogout,
                hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert,
            )
            .await
            .unwrap()
        );
        assert!(
            hypr_db_app::insert_cloudsync_recovery_barrier(db.pool(), &state, &key)
                .await
                .unwrap()
        );
        assert!(
            hypr_db_app::advance_cloudsync_recovery_phase(
                db.pool(),
                &generation,
                hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert,
                hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm,
            )
            .await
            .unwrap()
        );

        let config = hypr_db_core::CloudsyncRuntimeConfig {
            connection_string: "test-database".to_string(),
            auth: hypr_db_core::CloudsyncAuth::None,
            tables: vec![],
            sync_interval_ms: DEFAULT_CLOUDSYNC_INTERVAL_MS,
            wait_ms: Some(5_000),
            max_retries: Some(3),
        };
        db.cloudsync_prepare_manual_transport(config.clone())
            .await
            .unwrap();
        sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
            .bind(endpoint)
            .bind("full-resync-interrupt-test")
            .fetch_optional(db.pool())
            .await
            .unwrap();
        let auth_generation = runtime.cloudsync_auth_generation();
        runtime
            .schedule_cloudsync_full_resync(generation.clone(), config, auth_generation)
            .await;
        tokio::task::spawn_blocking(move || {
            stalled_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("full resync did not reach the stalled manual send");
        })
        .await
        .unwrap();

        tokio::time::timeout(
            std::time::Duration::from_millis(2_500),
            runtime.begin_cloudsync_activity_with_timeout(
                "capture".to_string(),
                "session-1".to_string(),
                std::time::Duration::from_millis(1_800),
            ),
        )
        .await
        .expect("activity did not drain the stalled full resync")
        .unwrap();
        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            runtime.execute(
                "INSERT INTO sessions (id, workspace_id, title)
                 VALUES ('local-after-recovery', 'workspace-1', 'Local')"
                    .to_string(),
                vec![],
            ),
        )
        .await
        .expect("local write remained blocked after full-resync cancellation")
        .unwrap();

        assert_eq!(
            hypr_db_app::cloudsync_recovery_state(db.pool())
                .await
                .unwrap()
                .unwrap()
                .phase,
            hypr_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm
        );
        assert!(
            runtime
                .scheduled_cloudsync_full_resync
                .lock()
                .unwrap()
                .is_active(&generation)
        );

        runtime.cancel_cloudsync_full_resync().await;
        runtime
            .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
            .await
            .unwrap();
        let _ = release_tx.send(());
        server.join().unwrap();
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

    async fn install_blocked_full_resync_cancellation(
        runtime: &PluginDbRuntime,
    ) -> (
        tokio::sync::oneshot::Sender<()>,
        tokio::sync::oneshot::Receiver<()>,
    ) {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (cancellation_started_tx, cancellation_started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let join_handle = tokio::spawn(async move {
            let _ = shutdown_rx.await;
            let _ = cancellation_started_tx.send(());
            let _ = release_rx.await;
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
        (release_tx, cancellation_started_rx)
    }

    async fn assert_auth_invalidation_cancels_inflight_configuration(logout: bool) {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = std::sync::Arc::new(PluginDbRuntime::new(db));
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        runtime
            .set_e2ee_recovery_key("user-a", &recovery_key)
            .unwrap();
        let (release_cancellation, cancellation_started) =
            install_blocked_full_resync_cancellation(runtime.as_ref()).await;

        let configure_runtime = std::sync::Arc::clone(&runtime);
        let configure = tokio::spawn(async move {
            configure_runtime
                .configure_cloudsync_token(
                    "managed-database-id".to_string(),
                    "stale-token".to_string(),
                    "user-a".to_string(),
                    crate::CloudsyncE2eeWitness {
                        endpoint: "https://example.com/witness".to_string(),
                        access_token: "access-token".to_string(),
                    },
                )
                .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), cancellation_started)
            .await
            .expect("token configuration did not enter full-resync cancellation")
            .unwrap();
        let configuration_generation = runtime.cloudsync_auth_generation();

        let invalidation_runtime = std::sync::Arc::clone(&runtime);
        let invalidation = tokio::spawn(async move {
            if logout {
                invalidation_runtime.logout_cloudsync(false).await
            } else {
                invalidation_runtime.suspend_cloudsync().await
            }
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while runtime.cloudsync_auth_generation() == configuration_generation {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("auth generation was not invalidated before waiting");

        release_cancellation
            .send(())
            .expect("failed to release full-resync cancellation");
        let error = tokio::time::timeout(std::time::Duration::from_secs(1), configure)
            .await
            .expect("cancelled token configuration did not finish")
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            error,
            crate::Error::CloudsyncConfigurationCancelled
        ));
        tokio::time::timeout(std::time::Duration::from_secs(1), invalidation)
            .await
            .expect("CloudSync invalidation did not finish")
            .unwrap()
            .unwrap();

        assert!(runtime.workspace_key("user-a").is_none());
        assert!(runtime.cloudsync_full_resync_task.lock().await.is_none());
        let status = runtime.cloudsync_status().await.unwrap();
        assert_eq!(status["configured"], false);
        assert_eq!(status["network_initialized"], false);
    }

    #[tokio::test]
    async fn suspend_invalidates_and_fails_closed_an_inflight_token_configuration() {
        assert_auth_invalidation_cancels_inflight_configuration(false).await;
    }

    #[tokio::test]
    async fn logout_invalidates_and_fails_closed_an_inflight_token_configuration() {
        assert_auth_invalidation_cancels_inflight_configuration(true).await;
    }

    async fn spawn_stalled_witness_configuration() -> (
        tempfile::TempDir,
        std::sync::Arc<PluginDbRuntime>,
        MockServer,
        tokio::task::JoinHandle<Result<crate::CloudsyncTokenConfigurationResult>>,
    ) {
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
        sqlx::query(
            "UPDATE storage_migration_state
             SET importer_version = ?, parity_verified = 1
             WHERE id = 'legacy_v1'",
        )
        .bind(hypr_db_app::LEGACY_IMPORTER_VERSION)
        .execute(db.pool())
        .await
        .unwrap();
        let runtime = std::sync::Arc::new(PluginDbRuntime::new(std::sync::Arc::new(db)));
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        runtime
            .set_e2ee_recovery_key("user-a", &recovery_key)
            .unwrap();
        assert!(
            runtime
                .bind_cloudsync_account("user-a".to_string())
                .await
                .unwrap()
        );
        let witness_server = MockServer::start().await;
        Mock::given(path("/sync/e2ee/witness/user-a"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_secs(60))
                    .set_body_json(serde_json::json!({
                        "initialized": true,
                        "initializedAt": "2026-07-23T00:00:00Z",
                        "headSequence": 0,
                        "throughSequence": 0,
                        "nextAfterSequence": 0,
                        "events": [],
                    })),
            )
            .mount(&witness_server)
            .await;
        let witness_endpoint = format!("{}/sync/e2ee/witness/user-a", witness_server.uri());

        let configure_runtime = std::sync::Arc::clone(&runtime);
        let configure = tokio::spawn(async move {
            configure_runtime
                .configure_cloudsync_token(
                    "managed-database-id".to_string(),
                    "stale-token".to_string(),
                    "user-a".to_string(),
                    crate::CloudsyncE2eeWitness {
                        endpoint: witness_endpoint,
                        access_token: "access-token".to_string(),
                    },
                )
                .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                if !witness_server
                    .received_requests()
                    .await
                    .expect("failed to inspect witness requests")
                    .is_empty()
                {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("token configuration did not reach the witness");
        (dir, runtime, witness_server, configure)
    }

    #[tokio::test]
    async fn suspend_cancels_a_stalled_witness_configuration() {
        let (_dir, runtime, _witness_server, configure) =
            spawn_stalled_witness_configuration().await;
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            runtime.suspend_cloudsync(),
        )
        .await
        .expect("suspension waited for the stalled witness request")
        .unwrap();
        let error = configure.await.unwrap().unwrap_err();
        assert!(matches!(
            error,
            crate::Error::CloudsyncConfigurationCancelled
        ));
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn suspend_drains_token_configuration_during_large_replica_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = std::sync::Arc::new(
            Db::open(DbOpenOptions {
                storage: DbStorage::Local(&db_path),
                cloudsync_enabled: true,
                journal_mode_wal: true,
                foreign_keys: true,
                max_connections: Some(4),
            })
            .await
            .unwrap(),
        );
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        sqlx::query(
            "UPDATE storage_migration_state
             SET importer_version = ?, parity_verified = 1
             WHERE id = 'legacy_v1'",
        )
        .bind(hypr_db_app::LEGACY_IMPORTER_VERSION)
        .execute(db.pool())
        .await
        .unwrap();
        hypr_db_app::claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        hypr_db_app::set_cloudsync_personal_write_scope(db.pool(), "user-a")
            .await
            .unwrap();
        sqlx::query(
            "WITH RECURSIVE rows(id) AS (
               SELECT 1
               UNION ALL
               SELECT id + 1 FROM rows WHERE id < 100000
             )
             INSERT INTO e2ee_records (id, workspace_id, payload)
             SELECT
               printf('record-%d', id),
               'user-a',
               printf('ciphertext-%d', id)
             FROM rows",
        )
        .execute(db.pool())
        .await
        .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            100_000
        );
        db.cloudsync_init(CLOUDSYNC_REPLICA_TABLE, None, None)
            .await
            .unwrap();
        db.cloudsync_set_filter(CLOUDSYNC_REPLICA_TABLE, CLOUDSYNC_WRITE_FILTER)
            .await
            .unwrap();

        let runtime = std::sync::Arc::new(PluginDbRuntime::new(std::sync::Arc::clone(&db)));
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        runtime
            .set_e2ee_recovery_key("user-a", &recovery_key)
            .unwrap();
        let key = runtime.workspace_key("user-a").unwrap();
        let generation =
            hypr_db_app::stage_cloudsync_poison_recovery(db.pool(), "user-a", "user-a")
                .await
                .unwrap();
        let recovery = hypr_db_app::ensure_cloudsync_recovery_state(
            db.pool(),
            &generation,
            "user-a",
            "user-a",
            &key,
        )
        .await
        .unwrap();
        assert_eq!(
            recovery.phase,
            hypr_db_app::CloudsyncRecoveryPhase::NeedFirstLogout
        );

        let witness_state = InitiallyUninitializedWitness::default();
        witness_state.initialized.store(true, Ordering::SeqCst);
        let witness_server = MockServer::start().await;
        Mock::given(path("/sync/e2ee/witness/user-a"))
            .respond_with(witness_state)
            .mount(&witness_server)
            .await;
        let witness_endpoint = format!("{}/sync/e2ee/witness/user-a", witness_server.uri());

        let configure_runtime = std::sync::Arc::clone(&runtime);
        let configure = tokio::spawn(async move {
            configure_runtime
                .configure_cloudsync_token(
                    "managed-database-id".to_string(),
                    "stale-token".to_string(),
                    "user-a".to_string(),
                    crate::CloudsyncE2eeWitness {
                        endpoint: witness_endpoint,
                        access_token: "access-token".to_string(),
                    },
                )
                .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while !db.cloudsync_interrupt_registered() {
                assert!(
                    !configure.is_finished(),
                    "token configuration finished before native replica cleanup"
                );
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("token configuration did not register native replica cleanup");

        let suspended_at = std::time::Instant::now();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            runtime.suspend_cloudsync(),
        )
        .await
        .expect("suspension did not drain native replica cleanup")
        .unwrap();
        assert!(
            suspended_at.elapsed() < std::time::Duration::from_secs(1),
            "suspension exceeded the cancellation deadline"
        );

        let error = configure.await.unwrap().unwrap_err();
        assert!(matches!(
            error,
            crate::Error::CloudsyncConfigurationCancelled
        ));
        assert!(!db.cloudsync_interrupt_registered());

        let idle_at = std::time::Instant::now();
        db.cloudsync_wait_for_sync_idle().await;
        assert!(
            idle_at.elapsed() < std::time::Duration::from_millis(250),
            "CloudSync worker was not idle after suspension"
        );
        db.cloudsync_version()
            .await
            .expect("CloudSync SQLite worker crashed during cancellation");
        let status = runtime.cloudsync_status().await.unwrap();
        assert_eq!(status["configured"], false);
        assert_eq!(status["running"], false);
        assert_eq!(status["network_initialized"], false);

        let inserted_at = std::time::Instant::now();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, title)
             VALUES ('local-after-cleanup-cancellation', 'user-a', 'Local')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        assert!(
            inserted_at.elapsed() < std::time::Duration::from_millis(250),
            "local write remained blocked after configuration cancellation"
        );
    }

    #[tokio::test]
    async fn activity_begin_cancels_and_drains_a_stalled_witness_configuration() {
        let (_dir, runtime, _witness_server, configure) =
            spawn_stalled_witness_configuration().await;

        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            runtime.begin_cloudsync_activity_with_timeout(
                "capture".to_string(),
                "session-1".to_string(),
                std::time::Duration::from_millis(500),
            ),
        )
        .await
        .expect("activity begin waited for the stalled witness request")
        .unwrap();
        let error = configure.await.unwrap().unwrap_err();
        assert!(matches!(error, crate::Error::CloudsyncActivityDeferred));
        assert!(runtime.e2ee_sync_hook.activity_paused());
        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('activity-write', 'user-a', 'user-a', 'Local write')",
            )
            .execute(runtime.pool()),
        )
        .await
        .expect("local write remained blocked after configuration drained")
        .unwrap();
    }

    #[tokio::test]
    async fn stale_configuration_waiting_after_a_newer_attempt_does_not_clear_its_key() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(db);
        let stale_generation = runtime.begin_cloudsync_auth_configuration();
        let _newer_generation = runtime.begin_cloudsync_auth_configuration();
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        runtime
            .set_e2ee_recovery_key("new-user", &recovery_key)
            .unwrap();

        let error = runtime
            .configure_cloudsync_token_with_projection_at_generation(
                CloudsyncTokenConfiguration::new(
                    "managed-database-id".to_string(),
                    "stale-token".to_string(),
                    "old-user".to_string(),
                    None,
                    crate::CloudsyncE2eeWitness {
                        endpoint: "https://example.com/witness".to_string(),
                        access_token: "access-token".to_string(),
                    },
                ),
                Some(("old-user".to_string(), recovery_key)),
                stale_generation,
            )
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            crate::Error::CloudsyncConfigurationCancelled
        ));
        assert!(runtime.workspace_key("new-user").is_some());
        assert!(runtime.workspace_key("old-user").is_none());
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
    async fn suspend_cancels_recovery_before_waiting_for_its_control_guard() {
        let runtime = PluginDbRuntime::new(std::sync::Arc::new(
            Db::connect_memory_plain().await.unwrap(),
        ));
        let control_operation = std::sync::Arc::clone(&runtime.cloudsync_control_operation);
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (control_acquired_tx, control_acquired_rx) = tokio::sync::oneshot::channel();
        let join_handle = tokio::spawn(async move {
            let _control_operation = control_operation.lock().await;
            let _ = control_acquired_tx.send(());
            let _ = shutdown_rx.await;
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
        tokio::time::timeout(std::time::Duration::from_secs(1), control_acquired_rx)
            .await
            .expect("recovery task did not acquire CloudSync control")
            .unwrap();

        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            runtime.suspend_cloudsync(),
        )
        .await
        .expect("suspension waited for recovery instead of cancelling it")
        .unwrap();
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
    async fn dropping_runtime_signals_full_resync_task_shutdown() {
        let runtime = PluginDbRuntime::new(std::sync::Arc::new(
            Db::connect_memory_plain().await.unwrap(),
        ));
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (shutdown_observed_tx, shutdown_observed_rx) = tokio::sync::oneshot::channel();
        let join_handle = tokio::spawn(async move {
            let _ = started_tx.send(());
            shutdown_rx.await.unwrap();
            let _ = shutdown_observed_tx.send(());
        });
        *runtime.cloudsync_full_resync_task.lock().await = Some(CloudsyncFullResyncTask {
            config: test_full_resync_config("test-token"),
            generation: "test-generation".to_string(),
            shutdown_tx: Some(shutdown_tx),
            join_handle,
        });
        started_rx.await.unwrap();

        drop(runtime);

        tokio::time::timeout(std::time::Duration::from_secs(1), shutdown_observed_rx)
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

        require_disposable_cloudsync_replica(
            runtime.db.as_ref(),
            CloudsyncOperationCancellation::None,
        )
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
            require_disposable_cloudsync_replica(
                runtime.db.as_ref(),
                CloudsyncOperationCancellation::None,
            )
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
                CloudsyncOperationCancellation::None,
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

        let cancellation = crate::e2ee_witness::E2eeWitnessCancellation::default();
        runtime
            .prepare_e2ee_cutover_and_initialize_witness(&witness, &workspace_key, &cancellation)
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
        let cancellation = crate::e2ee_witness::E2eeWitnessCancellation::default();

        runtime
            .prepare_cloudsync_config_fail_closed(
                hypr_db_core::CloudsyncRuntimeConfig {
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
                },
                &cancellation,
            )
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
    async fn cloudsync_status_does_not_treat_inbound_reconciliation_as_unsent() {
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
            "INSERT INTO e2ee_witness_records (
               workspace_id,
               record_id,
               revision,
               writer_id,
               payload_hash,
               payload,
               sequence
             )
             VALUES ('workspace-1', 'record-1', 1, 'writer-1', 'hash', 'payload', 1)",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let status = runtime.cloudsync_status().await.unwrap();

        assert!(runtime.e2ee_sync_hook.reconciliation_requested());
        assert_ne!(status["has_unsent_changes"], true);
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
    async fn repeated_cloudsync_status_polling_returns_base_status_on_a_busy_pool() {
        let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
        hypr_db_app::prepare_schema(db.as_ref()).await.unwrap();
        let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
        let mut held_connection = db.pool().acquire().await.unwrap();
        let started = std::time::Instant::now();

        for _ in 0..32 {
            let status = tokio::time::timeout(
                std::time::Duration::from_millis(50),
                runtime.cloudsync_status(),
            )
            .await
            .expect("CloudSync status waited for a busy pool")
            .unwrap();
            assert_eq!(status["activity_paused"], false);
            assert_eq!(status["has_unsent_changes"], serde_json::Value::Null);
            assert!(status.get("recovery_pending").is_none());
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(1));

        held_connection.return_to_pool().await;
        for _ in 0..32 {
            tokio::time::timeout(
                std::time::Duration::from_millis(100),
                runtime.cloudsync_status(),
            )
            .await
            .expect("CloudSync status left SQLite work in flight")
            .unwrap();
        }
        assert_eq!(db.pool().num_idle(), 1);
        tokio::time::timeout(std::time::Duration::from_millis(100), db.pool().acquire())
            .await
            .expect("repeated CloudSync status polling exhausted the pool")
            .unwrap();
    }
}

use std::collections::{HashMap, HashSet};

use super::E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT;
use super::sync_result::{cloudsync_receive_completed, cloudsync_receive_requires_reconciliation};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct CloudsyncActivity {
    activity: String,
    key: String,
}

pub(crate) struct CloudsyncTokenConfiguration {
    pub(super) database_id: String,
    pub(super) token: String,
    pub(super) account_user_id: String,
    pub(super) workspace_projection: Option<anlg_db_app::CloudsyncWorkspaceProjection>,
    pub(super) e2ee_witness: crate::CloudsyncE2eeWitness,
}

impl CloudsyncTokenConfiguration {
    pub(crate) fn new(
        database_id: String,
        token: String,
        account_user_id: String,
        workspace_projection: Option<anlg_db_app::CloudsyncWorkspaceProjection>,
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
pub(super) struct E2eeSyncHook {
    keys: std::sync::RwLock<HashMap<String, anlg_e2ee::WorkspaceKey>>,
    witness: std::sync::RwLock<Option<crate::e2ee_witness::E2eeWitnessClient>>,
    pub(super) witness_changed: tokio::sync::Notify,
    activities: std::sync::RwLock<HashSet<CloudsyncActivity>>,
    pub(super) activity_changed: tokio::sync::Notify,
    reconciliation_requested_epoch: std::sync::atomic::AtomicU64,
    reconciliation_completed_epoch: std::sync::atomic::AtomicU64,
    active_sync_generation: std::sync::atomic::AtomicU64,
    active_sync_cancellation:
        std::sync::Mutex<Option<(u64, crate::e2ee_witness::E2eeWitnessCancellation)>>,
    #[cfg(test)]
    pub(super) received_apply_cancellation_checks: std::sync::atomic::AtomicU64,
    #[cfg(test)]
    pub(super) full_resync_control_waits: std::sync::atomic::AtomicU64,
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
    pub(super) fn set_personal_workspace(
        &self,
        workspace_id: &str,
        recovery_key: &anlg_e2ee::RecoveryKey,
    ) -> std::result::Result<(), anlg_e2ee::Error> {
        let key = recovery_key.workspace_key(workspace_id)?;
        *self.keys.write().unwrap() = HashMap::from([(workspace_id.to_string(), key)]);
        self.request_reconciliation();
        Ok(())
    }

    pub(super) fn has_workspace(&self, workspace_id: &str) -> bool {
        self.keys.read().unwrap().contains_key(workspace_id)
    }

    pub(super) fn workspace_key(&self, workspace_id: &str) -> Option<anlg_e2ee::WorkspaceKey> {
        self.keys.read().unwrap().get(workspace_id).cloned()
    }

    pub(super) fn clear(&self) {
        self.keys.write().unwrap().clear();
        *self.witness.write().unwrap() = None;
        self.witness_changed.notify_waiters();
        let requested = self
            .reconciliation_requested_epoch
            .load(std::sync::atomic::Ordering::Acquire);
        self.reconciliation_completed_epoch
            .fetch_max(requested, std::sync::atomic::Ordering::AcqRel);
    }

    pub(super) fn clear_activities(&self) {
        let mut activities = self.activities.write().unwrap();
        activities.clear();
        drop(activities);
        self.activity_changed.notify_waiters();
    }

    pub(super) fn snapshot(&self) -> HashMap<String, anlg_e2ee::WorkspaceKey> {
        self.keys.read().unwrap().clone()
    }

    pub(super) fn set_witness(&self, witness: crate::e2ee_witness::E2eeWitnessClient) {
        *self.witness.write().unwrap() = Some(witness);
        self.witness_changed.notify_waiters();
        self.request_reconciliation();
    }

    pub(super) fn witness(&self) -> Option<crate::e2ee_witness::E2eeWitnessClient> {
        self.witness.read().unwrap().clone()
    }

    pub(super) fn begin_activity(&self, activity: String, key: String) {
        self.activities
            .write()
            .unwrap()
            .insert(CloudsyncActivity { activity, key });
        self.activity_changed.notify_waiters();
    }

    pub(super) fn end_activity(&self, activity: &str, key: &str) -> bool {
        let mut activities = self.activities.write().unwrap();
        let removed = activities.remove(&CloudsyncActivity {
            activity: activity.to_string(),
            key: key.to_string(),
        });
        removed && activities.is_empty()
    }

    pub(super) fn activity_paused(&self) -> bool {
        !self.activities.read().unwrap().is_empty()
    }

    pub(super) fn has_activity_lease(&self, activity: &str, key: &str) -> bool {
        self.activities
            .read()
            .unwrap()
            .contains(&CloudsyncActivity {
                activity: activity.to_string(),
                key: key.to_string(),
            })
    }

    pub(super) fn has_activity(&self, activity: &str) -> bool {
        self.activities
            .read()
            .unwrap()
            .iter()
            .any(|lease| lease.activity == activity)
    }

    pub(super) async fn wait_until_activity_resumed(&self) {
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

    pub(super) async fn wait_until_activity_paused(&self) {
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

    pub(super) fn notify_activity_changed(&self) {
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

    pub(super) fn request_reconciliation(&self) -> u64 {
        self.reconciliation_requested_epoch
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
            .checked_add(1)
            .expect("E2EE reconciliation epoch overflowed")
    }

    pub(super) fn reconciliation_request_epoch(&self) -> Option<u64> {
        let requested = self
            .reconciliation_requested_epoch
            .load(std::sync::atomic::Ordering::Acquire);
        let completed = self
            .reconciliation_completed_epoch
            .load(std::sync::atomic::Ordering::Acquire);
        (requested > completed).then_some(requested)
    }

    pub(super) fn complete_reconciliation(&self, epoch: u64) {
        self.reconciliation_completed_epoch
            .fetch_max(epoch, std::sync::atomic::Ordering::AcqRel);
    }

    pub(super) fn reconciliation_requested(&self) -> bool {
        self.reconciliation_request_epoch().is_some()
    }

    pub(super) async fn prepare_local_snapshot(
        &self,
        pool: &sqlx::SqlitePool,
        cancellation: &crate::e2ee_witness::E2eeWitnessCancellation,
    ) -> std::result::Result<(), anlg_db_app::E2eeReplicaError> {
        anlg_db_app::encrypt_e2ee_replica_changes_deferring_active_captures_cancellable(
            pool,
            &self.snapshot(),
            || cancellation.is_cancelled(),
        )
        .await
        .map(|_| ())
    }
}

impl anlg_db_core::CloudsyncSyncHook for E2eeSyncHook {
    fn activity_paused(&self) -> bool {
        self.activity_paused()
    }

    fn before_sync<'a>(
        &'a self,
        pool: &'a sqlx::SqlitePool,
    ) -> anlg_db_core::CloudsyncBeforeHookFuture<'a> {
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
                let full_resync_pending = anlg_db_app::cloudsync_full_resync_generation(pool)
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
                    return Ok(anlg_db_core::CloudsyncSyncDirective::ReceiveOnly);
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
                    anlg_db_app::encrypt_e2ee_replica_changes_bounded_deferring_active_captures_cancellable(
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
                Ok(anlg_db_core::CloudsyncSyncDirective::SendAndReceive)
            };
            tokio::pin!(operation);
            tokio::select! {
                biased;
                _ = self.wait_until_activity_paused() => {
                    cancellation.cancel();
                    let _ = operation.await;
                    Ok(anlg_db_core::CloudsyncSyncDirective::Deferred)
                }
                result = &mut operation => result,
            }
        })
    }

    fn after_sync<'a>(
        &'a self,
        pool: &'a sqlx::SqlitePool,
        result: &'a anlg_db_core::CloudsyncNetworkResult,
    ) -> anlg_db_core::CloudsyncHookFuture<'a> {
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
                let recovery_pending = anlg_db_app::cloudsync_full_resync_generation(pool)
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
                        anlg_db_app::apply_received_e2ee_replica_changes_with_witness_cancellable(
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
                    anlg_db_app::E2eeReplicaStats::default()
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
                    || anlg_db_app::has_pending_e2ee_dirty_rows_deferring_active_captures(
                        pool, &keys,
                    )
                    .await
                    .map_err(|error| {
                        std::io::Error::other(format!(
                            "failed to inspect pending E2EE replica changes: {error}"
                        ))
                    })?;
                cancellation.check()?;
                Ok(anlg_db_core::CloudsyncHookOutcome {
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
                    Ok(anlg_db_core::CloudsyncHookOutcome {
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

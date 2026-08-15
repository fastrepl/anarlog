use std::collections::{HashMap, HashSet};

use super::E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT;
use super::sync_result::{cloudsync_receive_completed, cloudsync_receive_requires_reconciliation};

#[derive(Clone, Copy, PartialEq, Eq)]
enum E2eeTransport {
    None = 0,
    Cloudsync = 1,
    Replica = 2,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct CloudsyncActivity {
    activity: String,
    key: String,
}

#[derive(Clone, Default)]
pub(super) struct ReplicaSyncStatus {
    pub(super) syncing: bool,
    pub(super) last_sync_at_ms: Option<u64>,
    pub(super) last_error: Option<String>,
    pub(super) consecutive_failures: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReplicaSyncOutcome {
    Settled,
    MoreWork,
    Paused,
}

fn ordered_witness_workspace_ids(
    keys: &HashMap<String, anlg_e2ee::WorkspaceKeyring>,
    witnesses: &HashMap<String, crate::e2ee_witness::E2eeWitnessClient>,
) -> std::io::Result<Vec<String>> {
    if keys.is_empty()
        || keys.len() != witnesses.len()
        || keys
            .keys()
            .any(|workspace_id| !witnesses.contains_key(workspace_id))
    {
        return Err(std::io::Error::other(
            "E2EE freshness witnesses do not match configured workspaces",
        ));
    }
    let mut workspace_ids = keys.keys().cloned().collect::<Vec<_>>();
    workspace_ids.sort_unstable();
    Ok(workspace_ids)
}

pub(crate) struct CloudsyncTokenConfiguration {
    pub(super) database_id: String,
    pub(super) token: String,
    pub(super) account_user_id: String,
    pub(super) workspace_projection: Option<anlg_db_app::CloudsyncWorkspaceProjection>,
    pub(super) e2ee_witness: crate::CloudsyncE2eeWitness,
}

pub(crate) struct E2eeWorkspaceKeyConfiguration {
    pub(super) personal_workspace_id: String,
    pub(super) recovery_key: anlg_e2ee::RecoveryKey,
    pub(super) shared_keyrings: HashMap<String, anlg_e2ee::WorkspaceKeyring>,
}

impl E2eeWorkspaceKeyConfiguration {
    pub(crate) fn new(
        personal_workspace_id: String,
        recovery_key: anlg_e2ee::RecoveryKey,
        shared_keyrings: HashMap<String, anlg_e2ee::WorkspaceKeyring>,
    ) -> Self {
        Self {
            personal_workspace_id,
            recovery_key,
            shared_keyrings,
        }
    }
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
    keys: std::sync::RwLock<HashMap<String, anlg_e2ee::WorkspaceKeyring>>,
    witnesses: std::sync::RwLock<HashMap<String, crate::e2ee_witness::E2eeWitnessClient>>,
    transport: std::sync::atomic::AtomicU8,
    pub(super) witness_changed: tokio::sync::Notify,
    pub(super) replica_sync_requested: tokio::sync::Notify,
    replica_status: std::sync::Mutex<ReplicaSyncStatus>,
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
        *self.keys.write().unwrap() = HashMap::from([(
            workspace_id.to_string(),
            anlg_e2ee::WorkspaceKeyring::new(key),
        )]);
        self.request_reconciliation();
        Ok(())
    }

    pub(super) fn set_workspaces(
        &self,
        personal_workspace_id: &str,
        recovery_key: &anlg_e2ee::RecoveryKey,
        mut shared_keyrings: HashMap<String, anlg_e2ee::WorkspaceKeyring>,
    ) -> std::result::Result<(), anlg_e2ee::Error> {
        shared_keyrings.insert(
            personal_workspace_id.to_string(),
            anlg_e2ee::WorkspaceKeyring::new(recovery_key.workspace_key(personal_workspace_id)?),
        );
        *self.keys.write().unwrap() = shared_keyrings;
        self.request_reconciliation();
        Ok(())
    }

    pub(super) fn has_workspace(&self, workspace_id: &str) -> bool {
        self.keys.read().unwrap().contains_key(workspace_id)
    }

    pub(super) fn workspace_key(&self, workspace_id: &str) -> Option<anlg_e2ee::WorkspaceKey> {
        self.keys
            .read()
            .unwrap()
            .get(workspace_id)
            .map(|keyring| keyring.active().clone())
    }

    pub(super) fn clear(&self) {
        self.cancel_active_sync();
        self.keys.write().unwrap().clear();
        self.witnesses.write().unwrap().clear();
        self.transport.store(
            E2eeTransport::None as u8,
            std::sync::atomic::Ordering::Release,
        );
        self.witness_changed.notify_waiters();
        self.replica_sync_requested.notify_waiters();
        *self.replica_status.lock().unwrap() = ReplicaSyncStatus::default();
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

    pub(super) fn snapshot(&self) -> HashMap<String, anlg_e2ee::WorkspaceKeyring> {
        self.keys.read().unwrap().clone()
    }

    #[cfg(test)]
    pub(super) fn set_witness(&self, witness: crate::e2ee_witness::E2eeWitnessClient) {
        self.set_witnesses_for_transport(
            HashMap::from([(witness.workspace_id().to_string(), witness)]),
            E2eeTransport::Cloudsync,
        );
    }

    pub(super) fn set_witnesses(
        &self,
        witnesses: HashMap<String, crate::e2ee_witness::E2eeWitnessClient>,
    ) {
        self.set_witnesses_for_transport(witnesses, E2eeTransport::Cloudsync);
    }

    pub(super) fn set_replica_witness(&self, witness: crate::e2ee_witness::E2eeWitnessClient) {
        self.set_witnesses_for_transport(
            HashMap::from([(witness.workspace_id().to_string(), witness)]),
            E2eeTransport::Replica,
        );
        self.replica_sync_requested.notify_one();
    }

    fn set_witnesses_for_transport(
        &self,
        witnesses: HashMap<String, crate::e2ee_witness::E2eeWitnessClient>,
        transport: E2eeTransport,
    ) {
        self.cancel_active_sync();
        *self.witnesses.write().unwrap() = witnesses;
        self.transport
            .store(transport as u8, std::sync::atomic::Ordering::Release);
        self.witness_changed.notify_waiters();
        self.request_reconciliation();
    }

    pub(super) fn replica_transport_configured(&self) -> bool {
        self.transport.load(std::sync::atomic::Ordering::Acquire) == E2eeTransport::Replica as u8
    }

    pub(super) fn request_replica_sync(&self) {
        self.replica_sync_requested.notify_one();
    }

    pub(super) fn replica_sync_started(&self) {
        self.replica_status.lock().unwrap().syncing = true;
    }

    pub(super) fn replica_sync_succeeded(&self) {
        let mut status = self.replica_status.lock().unwrap();
        status.syncing = false;
        status.last_sync_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok());
        status.last_error = None;
        status.consecutive_failures = 0;
    }

    pub(super) fn replica_sync_paused(&self) {
        self.replica_status.lock().unwrap().syncing = false;
    }

    pub(super) fn replica_sync_failed(&self, error: &std::io::Error) {
        let mut status = self.replica_status.lock().unwrap();
        status.syncing = false;
        status.last_error = Some(error.to_string());
        status.consecutive_failures = status.consecutive_failures.saturating_add(1);
    }

    pub(super) fn replica_status(&self) -> ReplicaSyncStatus {
        self.replica_status.lock().unwrap().clone()
    }

    pub(super) fn witness(&self) -> Option<crate::e2ee_witness::E2eeWitnessClient> {
        let witnesses = self.witnesses.read().unwrap();
        (witnesses.len() == 1)
            .then(|| witnesses.values().next().cloned())
            .flatten()
    }

    pub(super) fn witness_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Option<crate::e2ee_witness::E2eeWitnessClient> {
        self.witnesses.read().unwrap().get(workspace_id).cloned()
    }

    pub(super) fn witnesses(&self) -> HashMap<String, crate::e2ee_witness::E2eeWitnessClient> {
        self.witnesses.read().unwrap().clone()
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

    pub(super) async fn sync_replica_transport(
        &self,
        pool: &sqlx::SqlitePool,
    ) -> std::io::Result<ReplicaSyncOutcome> {
        if !self.replica_transport_configured() {
            return Ok(ReplicaSyncOutcome::Settled);
        }
        let keys = self.snapshot();
        let witness = self.witness();
        let active_sync = self.begin_sync();
        let cancellation = active_sync.cancellation.clone();
        let operation = async {
            cancellation.check()?;
            let witness = witness
                .ok_or_else(|| std::io::Error::other("E2EE replica service is not configured"))?;
            let keyring = keys
                .get(witness.workspace_id())
                .ok_or_else(|| std::io::Error::other("E2EE replica identity is not configured"))?;
            witness
                .refresh_notifying_cancellable(
                    pool,
                    keyring.active(),
                    || {
                        self.request_reconciliation();
                    },
                    &cancellation,
                )
                .await?;
            cancellation.check()?;
            anlg_db_app::encrypt_e2ee_replica_changes_bounded_deferring_active_captures_cancellable(
                pool,
                &keys,
                E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT,
                || cancellation.is_cancelled(),
            )
            .await
            .map_err(|error| {
                std::io::Error::other(format!("E2EE replica encryption failed: {error}"))
            })?;
            cancellation.check()?;
            witness
                .publish_and_refresh_notifying_cancellable(
                    pool,
                    keyring.active(),
                    || {
                        self.request_reconciliation();
                    },
                    &cancellation,
                )
                .await?;
            cancellation.check()?;
            let reconciliation_epoch = self.request_reconciliation();
            let stats = anlg_db_app::apply_received_e2ee_replica_changes_with_witness_cancellable(
                pool,
                &keys,
                true,
                || self.received_apply_cancelled(&cancellation),
            )
            .await
            .map_err(|error| {
                std::io::Error::other(format!("E2EE replica application failed: {error}"))
            })?;
            cancellation.check()?;
            if stats.remaining_replica_changes {
                self.request_reconciliation();
            }
            self.complete_reconciliation(reconciliation_epoch);
            let local_work_remaining = stats.remaining_replica_changes
                || self.reconciliation_requested()
                || anlg_db_app::has_pending_e2ee_dirty_rows_deferring_active_captures(pool, &keys)
                    .await
                    .map_err(|error| {
                        std::io::Error::other(format!(
                            "failed to inspect pending E2EE replica changes: {error}"
                        ))
                    })?;
            cancellation.check()?;
            Ok(local_work_remaining)
        };
        tokio::pin!(operation);
        tokio::select! {
            biased;
            _ = self.wait_until_activity_paused() => {
                cancellation.cancel();
                let _ = operation.await;
                Ok(ReplicaSyncOutcome::Paused)
            }
            result = &mut operation => result.map(|work_remaining| {
                if work_remaining {
                    ReplicaSyncOutcome::MoreWork
                } else {
                    ReplicaSyncOutcome::Settled
                }
            }),
        }
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
        let witnesses = self.witnesses();
        Box::pin(async move {
            let active_sync = self.begin_sync();
            let cancellation = active_sync.cancellation.clone();
            let operation = async {
                cancellation.check()?;
                let workspace_ids = ordered_witness_workspace_ids(&keys, &witnesses)?;
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
                for workspace_id in &workspace_ids {
                    witnesses[workspace_id]
                        .refresh_keyring_notifying_cancellable(
                            pool,
                            &keys[workspace_id],
                            || {
                                self.request_reconciliation();
                            },
                            &cancellation,
                        )
                        .await?;
                    cancellation.check()?;
                }
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
                for workspace_id in &workspace_ids {
                    witnesses[workspace_id]
                        .publish_and_refresh_keyring_notifying_cancellable(
                            pool,
                            &keys[workspace_id],
                            || {
                                self.request_reconciliation();
                            },
                            &cancellation,
                        )
                        .await?;
                    cancellation.check()?;
                }
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
        let witnesses = self.witnesses();
        Box::pin(async move {
            let active_sync = self.begin_sync();
            let cancellation = active_sync.cancellation.clone();
            let operation = async {
                cancellation.check()?;
                let workspace_ids = ordered_witness_workspace_ids(&keys, &witnesses)?;
                for workspace_id in &workspace_ids {
                    witnesses[workspace_id]
                        .refresh_keyring_notifying_cancellable(
                            pool,
                            &keys[workspace_id],
                            || {
                                self.request_reconciliation();
                            },
                            &cancellation,
                        )
                        .await?;
                    cancellation.check()?;
                }
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

use std::collections::{HashMap, HashSet};

use crate::witness::{E2eeWitnessCancellation, E2eeWitnessClient};

const E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT: i64 = 64;

fn cloudsync_receive_completed(result: &anlg_db_core::CloudsyncNetworkResult) -> bool {
    result.receive.as_ref().is_some_and(|receive| {
        receive.complete && receive.error.is_none() && receive.last_failure.is_none()
    })
}

fn cloudsync_receive_requires_reconciliation(
    result: &anlg_db_core::CloudsyncNetworkResult,
) -> bool {
    result.receive.as_ref().is_some_and(|receive| {
        receive.chunks > 0
            || (!receive.complete && receive.error.is_none() && receive.last_failure.is_none())
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum E2eeTransport {
    #[default]
    None,
    Cloudsync,
    Replica,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct CloudsyncActivity {
    activity: String,
    key: String,
}

#[derive(Clone, Default)]
pub struct ReplicaSyncStatus {
    pub syncing: bool,
    pub last_sync_at_ms: Option<u64>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplicaSyncOutcome {
    Settled,
    MoreWork,
    Paused,
}

fn ordered_witness_workspace_ids(
    keys: &HashMap<String, anlg_e2ee::WorkspaceKeyring>,
    witnesses: &HashMap<String, E2eeWitnessClient>,
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

// Keys, witnesses, and transport are replaced together under one lock so a
// sync operation can never observe a mixed-generation configuration.
#[derive(Clone, Default)]
struct E2eeRuntimeConfig {
    keys: HashMap<String, anlg_e2ee::WorkspaceKeyring>,
    witnesses: HashMap<String, E2eeWitnessClient>,
    transport: E2eeTransport,
}

#[derive(Default)]
pub struct E2eeSyncHook {
    config: std::sync::RwLock<E2eeRuntimeConfig>,
    pub witness_changed: tokio::sync::Notify,
    pub replica_sync_requested: tokio::sync::Notify,
    replica_status: std::sync::Mutex<ReplicaSyncStatus>,
    activities: std::sync::RwLock<HashSet<CloudsyncActivity>>,
    activity_epoch: std::sync::atomic::AtomicU64,
    pub activity_changed: tokio::sync::Notify,
    reconciliation_requested_epoch: std::sync::atomic::AtomicU64,
    reconciliation_completed_epoch: std::sync::atomic::AtomicU64,
    active_sync_generation: std::sync::atomic::AtomicU64,
    active_sync_cancellation: std::sync::Mutex<Option<(u64, E2eeWitnessCancellation)>>,
    pub received_apply_cancellation_checks: std::sync::atomic::AtomicU64,
    pub full_resync_control_waits: std::sync::atomic::AtomicU64,
}

struct ActiveE2eeSync<'a> {
    hook: &'a E2eeSyncHook,
    generation: u64,
    cancellation: E2eeWitnessCancellation,
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
    pub fn set_personal_workspace(
        &self,
        workspace_id: &str,
        recovery_key: &anlg_e2ee::RecoveryKey,
    ) -> std::result::Result<(), anlg_e2ee::Error> {
        let key = recovery_key.workspace_key(workspace_id)?;
        self.config.write().unwrap().keys = HashMap::from([(
            workspace_id.to_string(),
            anlg_e2ee::WorkspaceKeyring::new(key),
        )]);
        self.request_reconciliation();
        Ok(())
    }

    pub fn set_workspaces(
        &self,
        personal_workspace_id: &str,
        recovery_key: &anlg_e2ee::RecoveryKey,
        mut shared_keyrings: HashMap<String, anlg_e2ee::WorkspaceKeyring>,
    ) -> std::result::Result<(), anlg_e2ee::Error> {
        shared_keyrings.insert(
            personal_workspace_id.to_string(),
            anlg_e2ee::WorkspaceKeyring::new(recovery_key.workspace_key(personal_workspace_id)?),
        );
        self.config.write().unwrap().keys = shared_keyrings;
        self.request_reconciliation();
        Ok(())
    }

    pub fn has_workspace(&self, workspace_id: &str) -> bool {
        self.config.read().unwrap().keys.contains_key(workspace_id)
    }

    pub fn workspace_key(&self, workspace_id: &str) -> Option<anlg_e2ee::WorkspaceKey> {
        self.config
            .read()
            .unwrap()
            .keys
            .get(workspace_id)
            .map(|keyring| keyring.active().clone())
    }

    pub fn clear(&self) {
        self.cancel_active_sync();
        *self.config.write().unwrap() = E2eeRuntimeConfig::default();
        self.witness_changed.notify_waiters();
        self.replica_sync_requested.notify_waiters();
        *self.replica_status.lock().unwrap() = ReplicaSyncStatus::default();
        let requested = self
            .reconciliation_requested_epoch
            .load(std::sync::atomic::Ordering::Acquire);
        self.reconciliation_completed_epoch
            .fetch_max(requested, std::sync::atomic::Ordering::AcqRel);
    }

    pub fn clear_activities(&self) {
        let mut activities = self.activities.write().unwrap();
        activities.clear();
        drop(activities);
        self.activity_changed.notify_waiters();
    }

    pub fn snapshot(&self) -> HashMap<String, anlg_e2ee::WorkspaceKeyring> {
        self.config.read().unwrap().keys.clone()
    }

    fn config_snapshot(&self) -> E2eeRuntimeConfig {
        self.config.read().unwrap().clone()
    }

    pub fn set_witness(&self, witness: E2eeWitnessClient) {
        self.set_witnesses_for_transport(
            HashMap::from([(witness.workspace_id().to_string(), witness)]),
            E2eeTransport::Cloudsync,
        );
    }

    pub fn set_witnesses(&self, witnesses: HashMap<String, E2eeWitnessClient>) {
        self.set_witnesses_for_transport(witnesses, E2eeTransport::Cloudsync);
    }

    pub fn set_replica_witness(&self, witness: E2eeWitnessClient) {
        self.set_witnesses_for_transport(
            HashMap::from([(witness.workspace_id().to_string(), witness)]),
            E2eeTransport::Replica,
        );
        self.replica_sync_requested.notify_one();
    }

    fn set_witnesses_for_transport(
        &self,
        witnesses: HashMap<String, E2eeWitnessClient>,
        transport: E2eeTransport,
    ) {
        self.cancel_active_sync();
        {
            let mut config = self.config.write().unwrap();
            config.witnesses = witnesses;
            config.transport = transport;
        }
        self.witness_changed.notify_waiters();
        self.request_reconciliation();
    }

    pub fn replica_transport_configured(&self) -> bool {
        self.config.read().unwrap().transport == E2eeTransport::Replica
    }

    fn transport_configured(&self) -> bool {
        self.config.read().unwrap().transport != E2eeTransport::None
    }

    pub fn request_replica_sync(&self) {
        self.replica_sync_requested.notify_one();
    }

    pub fn replica_sync_started(&self) {
        self.replica_status.lock().unwrap().syncing = true;
    }

    pub fn replica_sync_succeeded(&self) {
        let mut status = self.replica_status.lock().unwrap();
        status.syncing = false;
        status.last_sync_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok());
        status.last_error = None;
        status.consecutive_failures = 0;
    }

    pub fn replica_sync_paused(&self) {
        self.replica_status.lock().unwrap().syncing = false;
    }

    pub fn replica_sync_failed(&self, error: &std::io::Error) {
        let mut status = self.replica_status.lock().unwrap();
        status.syncing = false;
        status.last_error = Some(error.to_string());
        status.consecutive_failures = status.consecutive_failures.saturating_add(1);
    }

    pub fn replica_status(&self) -> ReplicaSyncStatus {
        self.replica_status.lock().unwrap().clone()
    }

    pub fn witness(&self) -> Option<E2eeWitnessClient> {
        let config = self.config.read().unwrap();
        (config.witnesses.len() == 1)
            .then(|| config.witnesses.values().next().cloned())
            .flatten()
    }

    pub fn witness_for_workspace(&self, workspace_id: &str) -> Option<E2eeWitnessClient> {
        self.config
            .read()
            .unwrap()
            .witnesses
            .get(workspace_id)
            .cloned()
    }

    pub fn witnesses(&self) -> HashMap<String, E2eeWitnessClient> {
        self.config.read().unwrap().witnesses.clone()
    }

    pub fn begin_activity(&self, activity: String, key: String) {
        let inserted = self
            .activities
            .write()
            .unwrap()
            .insert(CloudsyncActivity { activity, key });
        if inserted {
            self.activity_epoch
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        }
        self.activity_changed.notify_waiters();
    }

    pub fn end_activity(&self, activity: &str, key: &str) -> bool {
        let mut activities = self.activities.write().unwrap();
        let removed = activities.remove(&CloudsyncActivity {
            activity: activity.to_string(),
            key: key.to_string(),
        });
        removed && activities.is_empty()
    }

    pub fn activity_paused(&self) -> bool {
        !self.activities.read().unwrap().is_empty()
    }

    pub fn activity_epoch(&self) -> u64 {
        self.activity_epoch
            .load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn has_activity_lease(&self, activity: &str, key: &str) -> bool {
        self.activities
            .read()
            .unwrap()
            .contains(&CloudsyncActivity {
                activity: activity.to_string(),
                key: key.to_string(),
            })
    }

    pub fn has_activity(&self, activity: &str) -> bool {
        self.activities
            .read()
            .unwrap()
            .iter()
            .any(|lease| lease.activity == activity)
    }

    pub async fn wait_until_activity_resumed(&self) {
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

    pub async fn wait_until_activity_paused(&self) {
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

    pub fn notify_activity_changed(&self) {
        self.activity_changed.notify_waiters();
    }

    fn begin_sync(&self) -> ActiveE2eeSync<'_> {
        let generation = self
            .active_sync_generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
            .wrapping_add(1);
        let cancellation = E2eeWitnessCancellation::default();
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

    fn received_apply_cancelled(&self, cancellation: &E2eeWitnessCancellation) -> bool {
        self.received_apply_cancellation_checks
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        cancellation.is_cancelled()
    }

    pub fn request_reconciliation(&self) -> u64 {
        self.reconciliation_requested_epoch
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
            .checked_add(1)
            .expect("E2EE reconciliation epoch overflowed")
    }

    pub fn reconciliation_request_epoch(&self) -> Option<u64> {
        let requested = self
            .reconciliation_requested_epoch
            .load(std::sync::atomic::Ordering::Acquire);
        let completed = self
            .reconciliation_completed_epoch
            .load(std::sync::atomic::Ordering::Acquire);
        (requested > completed).then_some(requested)
    }

    pub fn complete_reconciliation(&self, epoch: u64) {
        self.reconciliation_completed_epoch
            .fetch_max(epoch, std::sync::atomic::Ordering::AcqRel);
    }

    pub fn reconciliation_requested(&self) -> bool {
        self.reconciliation_request_epoch().is_some()
    }

    pub async fn prepare_local_snapshot(
        &self,
        pool: &sqlx::SqlitePool,
        cancellation: &E2eeWitnessCancellation,
    ) -> std::result::Result<(), anlg_db_app::E2eeReplicaError> {
        anlg_db_app::encrypt_e2ee_replica_changes_deferring_active_captures_cancellable(
            pool,
            &self.snapshot(),
            || cancellation.is_cancelled(),
        )
        .await
        .map(|_| ())
    }

    pub async fn sync_replica_transport(
        &self,
        pool: &sqlx::SqlitePool,
    ) -> std::io::Result<ReplicaSyncOutcome> {
        let config = self.config_snapshot();
        if config.transport != E2eeTransport::Replica {
            return Ok(ReplicaSyncOutcome::Settled);
        }
        let keys = config.keys;
        let witness = (config.witnesses.len() == 1)
            .then(|| config.witnesses.into_values().next())
            .flatten();
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
        self.transport_configured() && self.activity_paused()
    }

    fn before_sync<'a>(
        &'a self,
        pool: &'a sqlx::SqlitePool,
    ) -> anlg_db_core::CloudsyncBeforeHookFuture<'a> {
        let config = self.config_snapshot();
        if config.transport == E2eeTransport::None {
            return Box::pin(async { Ok(anlg_db_core::CloudsyncSyncDirective::default()) });
        }
        let keys = config.keys;
        let witnesses = config.witnesses;
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
        let config = self.config_snapshot();
        if config.transport == E2eeTransport::None {
            return Box::pin(async { Ok(anlg_db_core::CloudsyncHookOutcome::default()) });
        }
        if cloudsync_receive_requires_reconciliation(result) {
            self.request_reconciliation();
        }
        let keys = config.keys;
        let witnesses = config.witnesses;
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use anlg_db_core::CloudsyncSyncHook;

    use super::{E2eeSyncHook, E2eeTransport};
    use crate::witness::{E2eeWitnessClient, E2eeWitnessConfig};

    const TEST_RECOVERY_KEY: &str = "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc";

    fn test_witness(workspace_id: &str) -> E2eeWitnessClient {
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("http://127.0.0.1:9/sync/e2ee/witness/{workspace_id}"),
                access_token: "access-token".to_string(),
            },
            workspace_id,
        )
        .unwrap()
    }

    #[test]
    fn clear_resets_the_whole_configuration() {
        let hook = E2eeSyncHook::default();
        let recovery_key = anlg_e2ee::RecoveryKey::parse(TEST_RECOVERY_KEY).unwrap();
        hook.set_personal_workspace("workspace-1", &recovery_key)
            .unwrap();
        hook.set_witness(test_witness("workspace-1"));
        assert!(hook.has_workspace("workspace-1"));
        assert!(hook.witness().is_some());
        assert!(hook.reconciliation_requested());

        hook.clear();

        let config = hook.config_snapshot();
        assert!(config.keys.is_empty());
        assert!(config.witnesses.is_empty());
        assert_eq!(config.transport, E2eeTransport::None);
        assert!(!hook.reconciliation_requested());
    }

    // Under the previous layout witnesses and transport lived in separate
    // primitives, so a reader could observe a fresh witness map with a stale
    // transport flag. One lock makes that unrepresentable.
    #[test]
    fn reconfiguration_keeps_witnesses_and_transport_coherent() {
        let hook = std::sync::Arc::new(E2eeSyncHook::default());
        let start = std::sync::Arc::new(std::sync::Barrier::new(2));

        let writer = {
            let hook = std::sync::Arc::clone(&hook);
            let start = std::sync::Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                for _ in 0..500 {
                    hook.set_witnesses(HashMap::from([(
                        "cloudsync-ws".to_string(),
                        test_witness("cloudsync-ws"),
                    )]));
                    hook.set_replica_witness(test_witness("replica-ws"));
                }
            })
        };

        start.wait();
        for _ in 0..2000 {
            let config = hook.config_snapshot();
            match config.transport {
                E2eeTransport::None => assert!(config.witnesses.is_empty()),
                E2eeTransport::Cloudsync => {
                    assert!(config.witnesses.contains_key("cloudsync-ws"));
                }
                E2eeTransport::Replica => {
                    assert!(config.witnesses.contains_key("replica-ws"));
                }
            }
        }
        writer.join().unwrap();
    }

    #[tokio::test]
    async fn mismatched_keys_and_witnesses_fail_before_sync() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let hook = E2eeSyncHook::default();
        let recovery_key = anlg_e2ee::RecoveryKey::parse(TEST_RECOVERY_KEY).unwrap();
        hook.set_workspaces(
            "workspace-1",
            &recovery_key,
            HashMap::from([(
                "workspace-2".to_string(),
                anlg_e2ee::WorkspaceKeyring::new(
                    recovery_key.workspace_key("workspace-2").unwrap(),
                ),
            )]),
        )
        .unwrap();
        hook.set_witness(test_witness("workspace-1"));

        let error = hook.before_sync(&pool).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("witnesses do not match configured workspaces")
        );
    }

    #[tokio::test]
    async fn unconfigured_hook_is_transparent_to_cloudsync() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let hook = E2eeSyncHook::default();
        hook.begin_activity("capture".to_string(), "session-a".to_string());

        assert!(!CloudsyncSyncHook::activity_paused(&hook));
        assert_eq!(
            hook.before_sync(&pool).await.unwrap(),
            anlg_db_core::CloudsyncSyncDirective::default()
        );
        assert_eq!(
            hook.after_sync(&pool, &anlg_db_core::CloudsyncNetworkResult::default())
                .await
                .unwrap(),
            anlg_db_core::CloudsyncHookOutcome::default()
        );
        assert!(!hook.reconciliation_requested());
    }
}

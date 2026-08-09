use anlg_db_core::Db;

use super::sync_result::{
    cloudsync_receive_delivered, cloudsync_receive_delivered_final, cloudsync_send_completed,
    cloudsync_send_made_progress,
};
use super::{
    CLOUDSYNC_WRITE_FILTER, CloudsyncOperationCancellation, E2EE_CLOUDSYNC_DIRTY_ROW_LIMIT,
    PluginDbRuntime, cloudsync_recovery_cancelled, wait_until_cloudsync_auth_generation_changes,
};
use crate::Result;

pub(super) const CLOUDSYNC_FULL_RESYNC_PROGRESS_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(200);
pub(super) const CLOUDSYNC_FULL_RESYNC_RETRY_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(5);
pub(super) const CLOUDSYNC_RECOVERY_DELAYED_AFTER: std::time::Duration =
    std::time::Duration::from_secs(60);

pub(super) const CLOUDSYNC_REPLICA_TABLE: &str = "e2ee_records";
const E2EE_CLOUDSYNC_WITNESS_REPAIR_BYTE_LIMIT: usize = 16 * 1024 * 1024;

pub(super) struct CloudsyncFullResyncTask {
    #[cfg(test)]
    pub(super) config: anlg_db_core::CloudsyncRuntimeConfig,
    #[cfg(test)]
    pub(super) generation: String,
    pub(super) shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    pub(super) join_handle: tokio::task::JoinHandle<()>,
}

#[derive(Default)]
pub(super) struct CloudsyncFullResyncSchedule {
    pub(super) generation: Option<String>,
    pub(super) recovery_delayed: bool,
    pub(super) last_progress_at: Option<std::time::Instant>,
}

#[derive(Clone, Copy)]
pub(super) enum CloudsyncRecoveryStep {
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

pub(super) fn cloudsync_recovery_step_delay(
    step: CloudsyncRecoveryStep,
) -> Option<std::time::Duration> {
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
    pub(super) fn claim(&mut self, generation: &str) {
        self.generation = Some(generation.to_string());
        self.recovery_delayed = false;
        self.last_progress_at = Some(std::time::Instant::now());
    }

    pub(super) fn is_active(&self, generation: &str) -> bool {
        self.generation.as_deref() == Some(generation)
    }

    pub(super) fn cancel(&mut self) {
        self.generation = None;
        self.recovery_delayed = false;
        self.last_progress_at = None;
    }

    fn complete(&mut self, generation: &str) {
        if self.is_active(generation) {
            self.cancel();
        }
    }

    pub(super) fn mark_progress(&mut self, generation: &str) {
        if self.is_active(generation) {
            self.recovery_delayed = false;
            self.last_progress_at = Some(std::time::Instant::now());
        }
    }

    pub(super) fn mark_failure(&mut self, generation: &str) {
        if self.is_active(generation) {
            self.recovery_delayed = true;
        }
    }

    pub(super) fn mark_activity_resumed(&mut self) {
        if self.generation.is_some() {
            self.last_progress_at = Some(std::time::Instant::now());
        }
    }

    pub(super) fn is_delayed(&self, generation: &str) -> bool {
        self.is_active(generation)
            && (self.recovery_delayed
                || self.last_progress_at.is_some_and(|last_progress_at| {
                    last_progress_at.elapsed() >= CLOUDSYNC_RECOVERY_DELAYED_AFTER
                }))
    }
}

impl PluginDbRuntime {
    pub(super) async fn schedule_cloudsync_full_resync(
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
                            if apply.applied_fields > 0 {
                                return Ok(CloudsyncRecoveryStep::Progressed);
                            }
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
                            if repair.repaired_records > 0 {
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

    pub(super) async fn cancel_cloudsync_full_resync(&self) {
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

pub(super) async fn require_disposable_cloudsync_replica(
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

pub(super) async fn prepare_cloudsync_poison_recovery(
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

pub(super) async fn discard_cloudsync_recovery_replica(
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

pub(super) fn cloudsync_recovery_snapshot_ready(
    barrier_is_exact: bool,
    result: &anlg_db_core::CloudsyncNetworkResult,
) -> bool {
    barrier_is_exact && cloudsync_receive_delivered_final(result)
}

pub(super) fn is_permanent_cloudsync_workspace_rejection(
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

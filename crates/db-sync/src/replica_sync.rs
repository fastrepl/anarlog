use std::sync::Arc;

use anlg_db_core::Db;

use super::e2ee_sync::{E2eeSyncHook, ReplicaSyncEvent, ReplicaSyncOutcome, ReplicaSyncTrigger};

const REPLICA_SYNC_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
const REPLICA_SYNC_HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(300);
const REPLICA_SYNC_LOCAL_WORK_CHECK: std::time::Duration = std::time::Duration::from_secs(3);
const REPLICA_SYNC_RETRY: std::time::Duration = std::time::Duration::from_secs(5);
const REPLICA_SYNC_PACING: std::time::Duration = std::time::Duration::from_millis(50);

pub struct ReplicaSyncTask {
    _shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

pub fn spawn_replica_sync(db: Arc<Db>, hook: Arc<E2eeSyncHook>) -> ReplicaSyncTask {
    spawn_replica_sync_with_intervals(
        db,
        hook,
        REPLICA_SYNC_INTERVAL,
        REPLICA_SYNC_HEARTBEAT_INTERVAL,
    )
}

pub(crate) fn spawn_replica_sync_with_intervals(
    db: Arc<Db>,
    hook: Arc<E2eeSyncHook>,
    fallback_interval: std::time::Duration,
    heartbeat_interval: std::time::Duration,
) -> ReplicaSyncTask {
    spawn_replica_sync_with_test_intervals(
        db,
        hook,
        fallback_interval,
        heartbeat_interval,
        REPLICA_SYNC_LOCAL_WORK_CHECK,
    )
}

pub(crate) fn spawn_replica_sync_with_test_intervals(
    db: Arc<Db>,
    hook: Arc<E2eeSyncHook>,
    fallback_interval: std::time::Duration,
    heartbeat_interval: std::time::Duration,
    local_work_check_interval: std::time::Duration,
) -> ReplicaSyncTask {
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    super::spawn_background(async move {
        let mut legacy_cloudsync_retired = !db.cloudsync_enabled();
        let mut waiting_for_remote = false;
        let mut active_trigger: Option<ReplicaSyncTrigger> = None;
        let mut active_transferred = false;
        loop {
            let witness_changed = hook.witness_changed.notified();
            tokio::pin!(witness_changed);
            witness_changed.as_mut().enable();
            if !hook.replica_transport_configured() {
                active_trigger = None;
                active_transferred = false;
                tokio::select! {
                    _ = &mut shutdown_rx => return,
                    () = &mut witness_changed => {}
                }
                continue;
            }

            if hook.activity_paused() {
                tokio::select! {
                    _ = &mut shutdown_rx => return,
                    () = &mut witness_changed => {}
                    () = hook.wait_until_activity_resumed() => {}
                }
                continue;
            }

            let requested = hook.replica_sync_requested.notified();
            tokio::pin!(requested);
            requested.as_mut().enable();
            let mut witness_reset = false;
            let wait_started = tokio::time::Instant::now();
            loop {
                let health_changed = hook.watch_health_changed.notified();
                tokio::pin!(health_changed);
                health_changed.as_mut().enable();
                // /wait delivers prompt remote change notifications; relax periodic polling to a
                // heartbeat once connected while still picking up local SQLite writes promptly.
                let interval = if hook.witness_watch_healthy() {
                    heartbeat_interval
                } else {
                    fallback_interval
                };
                let remaining = interval.saturating_sub(wait_started.elapsed());
                if remaining.is_zero() {
                    break;
                }
                let local_check_step = if waiting_for_remote {
                    remaining
                } else {
                    local_work_check_interval.min(remaining)
                };
                tokio::select! {
                    biased;
                    _ = &mut shutdown_rx => return,
                    () = &mut witness_changed => {
                        witness_reset = true;
                        break;
                    }
                    () = &mut requested => break,
                    () = &mut health_changed => continue,
                    () = tokio::time::sleep(local_check_step) => {
                        if wait_started.elapsed() >= interval
                            || (!waiting_for_remote && hook.has_local_replica_work(db.pool()).await)
                        {
                            break;
                        }
                    }
                }
            }
            if witness_reset {
                waiting_for_remote = false;
                active_trigger = None;
                active_transferred = false;
                continue;
            }

            let generation = hook.config_generation();
            if hook.take_manual_sync_requested() {
                active_trigger = Some(ReplicaSyncTrigger::Manual);
            } else if active_trigger.is_none() {
                active_trigger = Some(ReplicaSyncTrigger::Background);
            }
            hook.replica_sync_started();
            let result = hook.sync_replica_transport_with_activity(db.pool()).await;
            if !hook.replica_transport_configured() || hook.config_generation() != generation {
                waiting_for_remote = false;
                active_trigger = None;
                active_transferred = false;
                continue;
            }
            match result {
                Ok((ReplicaSyncOutcome::MoreWork, transferred)) => {
                    waiting_for_remote = false;
                    active_transferred |= transferred;
                    hook.replica_sync_pending();
                    tokio::select! {
                        _ = &mut shutdown_rx => return,
                        () = tokio::time::sleep(REPLICA_SYNC_PACING) => {
                            hook.request_replica_sync();
                        }
                    }
                }
                Ok((ReplicaSyncOutcome::Settled, transferred)) => {
                    waiting_for_remote = false;
                    let did_transfer = active_transferred || transferred;
                    active_transferred = false;
                    if !legacy_cloudsync_retired {
                        match anlg_db_app::retire_legacy_cloudsync_tables(&db).await {
                            Ok(retired) => {
                                if retired > 0 {
                                    tracing::info!(
                                        retired,
                                        "retired legacy cloudsync table metadata"
                                    );
                                }
                                legacy_cloudsync_retired = true;
                            }
                            Err(error) => {
                                tracing::debug!(
                                    %error,
                                    "deferred legacy cloudsync table retirement"
                                );
                            }
                        }
                    }
                    let trigger = active_trigger
                        .take()
                        .unwrap_or(ReplicaSyncTrigger::Background);
                    hook.replica_sync_succeeded();
                    if trigger == ReplicaSyncTrigger::Manual || did_transfer {
                        hook.emit_replica_event(ReplicaSyncEvent::Completed { trigger });
                    }
                }
                Ok((ReplicaSyncOutcome::WaitingForRemote, transferred)) => {
                    waiting_for_remote = true;
                    active_transferred |= transferred;
                    hook.replica_sync_pending();
                }
                Ok((ReplicaSyncOutcome::Paused, _)) => {
                    waiting_for_remote = false;
                    active_trigger = None;
                    active_transferred = false;
                    hook.replica_sync_paused();
                }
                Err(error) => {
                    waiting_for_remote = false;
                    active_transferred = false;
                    let trigger = active_trigger
                        .take()
                        .unwrap_or(ReplicaSyncTrigger::Background);
                    hook.replica_sync_failed(&error);
                    hook.emit_replica_event(ReplicaSyncEvent::Failed { trigger });
                    tracing::warn!(%error, "encrypted replica sync failed");
                    tokio::select! {
                        _ = &mut shutdown_rx => return,
                        () = tokio::time::sleep(REPLICA_SYNC_RETRY) => {
                            hook.request_replica_sync();
                        }
                        () = &mut witness_changed => {}
                    }
                }
            }
        }
    });
    ReplicaSyncTask {
        _shutdown_tx: shutdown_tx,
    }
}

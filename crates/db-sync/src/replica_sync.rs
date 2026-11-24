use std::sync::Arc;

use anlg_db_core::Db;

use super::e2ee_sync::{E2eeSyncHook, ReplicaSyncOutcome};

const REPLICA_SYNC_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
const REPLICA_SYNC_HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(300);
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
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    super::spawn_background(async move {
        let mut legacy_cloudsync_retired = !db.cloudsync_enabled();
        loop {
            let witness_changed = hook.witness_changed.notified();
            tokio::pin!(witness_changed);
            witness_changed.as_mut().enable();
            if !hook.replica_transport_configured() {
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
            loop {
                let health_changed = hook.watch_health_changed.notified();
                tokio::pin!(health_changed);
                health_changed.as_mut().enable();
                // /wait delivers prompt remote change notifications; relax periodic polling to a heartbeat once connected.
                let interval = if hook.witness_watch_healthy() {
                    heartbeat_interval
                } else {
                    fallback_interval
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
                    () = tokio::time::sleep(interval) => break,
                }
            }
            if witness_reset {
                continue;
            }

            hook.replica_sync_started();
            let result = hook.sync_replica_transport(db.pool()).await;
            if !hook.replica_transport_configured() {
                continue;
            }
            match result {
                Ok(ReplicaSyncOutcome::MoreWork) => {
                    hook.replica_sync_pending();
                    tokio::select! {
                        _ = &mut shutdown_rx => return,
                        () = tokio::time::sleep(REPLICA_SYNC_PACING) => {
                            hook.request_replica_sync();
                        }
                    }
                }
                Ok(ReplicaSyncOutcome::Settled) => {
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
                    hook.replica_sync_succeeded();
                }
                Ok(ReplicaSyncOutcome::WaitingForRemote) => {
                    hook.replica_sync_pending();
                }
                Ok(ReplicaSyncOutcome::Paused) => hook.replica_sync_paused(),
                Err(error) => {
                    hook.replica_sync_failed(&error);
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

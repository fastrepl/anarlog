use std::sync::Arc;

use anlg_db_core::Db;

use super::e2ee_sync::{E2eeSyncHook, ReplicaSyncOutcome};

const REPLICA_SYNC_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
const REPLICA_SYNC_RETRY: std::time::Duration = std::time::Duration::from_secs(5);
const REPLICA_SYNC_PACING: std::time::Duration = std::time::Duration::from_millis(50);

pub struct ReplicaSyncTask {
    _shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

pub fn spawn_replica_sync(db: Arc<Db>, hook: Arc<E2eeSyncHook>) -> ReplicaSyncTask {
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
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
            tokio::select! {
                _ = &mut shutdown_rx => return,
                () = &mut witness_changed => continue,
                () = &mut requested => {}
                () = tokio::time::sleep(REPLICA_SYNC_INTERVAL) => {}
            }

            hook.replica_sync_started();
            let result = hook.sync_replica_transport(db.pool()).await;
            if !hook.replica_transport_configured() {
                continue;
            }
            match result {
                Ok(ReplicaSyncOutcome::MoreWork) => {
                    hook.replica_sync_succeeded();
                    tokio::select! {
                        _ = &mut shutdown_rx => return,
                        () = tokio::time::sleep(REPLICA_SYNC_PACING) => {
                            hook.request_replica_sync();
                        }
                    }
                }
                Ok(ReplicaSyncOutcome::Settled) => {
                    hook.replica_sync_succeeded();
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

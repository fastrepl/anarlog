use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anlg_db_core::Db;
use futures_util::StreamExt;

use super::e2ee_sync::E2eeSyncHook;
use crate::witness::{E2eeWitnessCancellation, E2eeWitnessClient};

const WATCH_POLL_PACING: std::time::Duration = std::time::Duration::from_secs(1);
const WATCH_ERROR_BACKOFF_MIN: std::time::Duration = std::time::Duration::from_secs(5);
const WATCH_ERROR_BACKOFF_MAX: std::time::Duration = std::time::Duration::from_secs(300);

pub struct WitnessWatchTask {
    _shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

/// Long-polls the witness service so remote changes trigger a sync round
/// promptly instead of waiting for the next background interval. The task
/// parks while no witness is configured and follows witness swaps via the
/// hook's change notifications; dropping the returned handle stops it.
pub fn spawn_witness_watch(db: Arc<Db>, hook: Arc<E2eeSyncHook>) -> WitnessWatchTask {
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    super::spawn_background(async move {
        loop {
            let witness_changed = hook.witness_changed.notified();
            tokio::pin!(witness_changed);
            witness_changed.as_mut().enable();
            let witnesses = hook.witnesses();
            if witnesses.is_empty() {
                hook.set_witness_watch_healthy(false);
                tokio::select! {
                    _ = &mut shutdown_rx => return,
                    () = &mut witness_changed => {}
                }
                continue;
            }
            hook.set_witness_watch_healthy(false);
            let health: Arc<Mutex<HashMap<String, bool>>> = Arc::new(Mutex::new(
                witnesses
                    .keys()
                    .map(|workspace_id| (workspace_id.clone(), false))
                    .collect(),
            ));
            let mut workers = futures_util::stream::FuturesUnordered::new();
            for witness in witnesses.into_values() {
                workers.push(watch_witness(
                    Arc::clone(&db),
                    Arc::clone(&hook),
                    Arc::clone(&health),
                    witness,
                ));
            }
            tokio::select! {
                _ = &mut shutdown_rx => return,
                () = &mut witness_changed => {
                    hook.set_witness_watch_healthy(false);
                    continue;
                }
                _ = workers.next() => {
                    hook.set_witness_watch_healthy(false);
                }
            }
        }
    });
    WitnessWatchTask {
        _shutdown_tx: shutdown_tx,
    }
}

fn update_workspace_watch_health(
    hook: &E2eeSyncHook,
    health: &Mutex<HashMap<String, bool>>,
    workspace_id: &str,
    healthy: bool,
) {
    let all_healthy = {
        let mut guard = health.lock().unwrap();
        guard.insert(workspace_id.to_string(), healthy);
        !guard.is_empty() && guard.values().all(|value| *value)
    };
    hook.set_witness_watch_healthy(all_healthy);
}

async fn watch_witness(
    db: Arc<Db>,
    hook: Arc<E2eeSyncHook>,
    health: Arc<Mutex<HashMap<String, bool>>>,
    witness: E2eeWitnessClient,
) {
    let mut error_backoff = WATCH_ERROR_BACKOFF_MIN;
    let mut last_triggered = 0_u64;
    let mut first_poll = true;
    loop {
        if first_poll {
            first_poll = false;
        } else {
            tokio::time::sleep(WATCH_POLL_PACING).await;
        }
        let cursor = match anlg_db_app::e2ee_witness_cursor(db.pool(), witness.workspace_id()).await
        {
            Ok(cursor) => cursor,
            Err(error) => {
                update_workspace_watch_health(&hook, &health, witness.workspace_id(), false);
                tracing::debug!(
                    workspace_id = witness.workspace_id(),
                    %error,
                    "witness watch could not read the local cursor"
                );
                tokio::time::sleep(error_backoff).await;
                error_backoff = (error_backoff * 2).min(WATCH_ERROR_BACKOFF_MAX);
                continue;
            }
        };
        let cancellation = E2eeWitnessCancellation::default();
        match witness
            .wait_for_remote_head(cursor.max(last_triggered), &cancellation)
            .await
        {
            Ok(Some(head)) => {
                update_workspace_watch_health(&hook, &health, witness.workspace_id(), true);
                last_triggered = head;
                error_backoff = WATCH_ERROR_BACKOFF_MIN;
                if hook.replica_transport_configured() {
                    hook.request_replica_sync();
                } else {
                    db.cloudsync_request_sync();
                }
            }
            Ok(None) => {
                update_workspace_watch_health(&hook, &health, witness.workspace_id(), true);
                error_backoff = WATCH_ERROR_BACKOFF_MIN;
            }
            Err(error) => {
                update_workspace_watch_health(&hook, &health, witness.workspace_id(), false);
                tracing::debug!(
                    workspace_id = witness.workspace_id(),
                    %error,
                    "witness watch long-poll failed"
                );
                tokio::time::sleep(error_backoff).await;
                error_backoff = (error_backoff * 2).min(WATCH_ERROR_BACKOFF_MAX);
            }
        }
    }
}

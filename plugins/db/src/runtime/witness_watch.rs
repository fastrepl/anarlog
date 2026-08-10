use std::sync::Arc;

use anlg_db_core::Db;

use super::e2ee_sync::E2eeSyncHook;
use crate::e2ee_witness::E2eeWitnessCancellation;

const WATCH_POLL_PACING: std::time::Duration = std::time::Duration::from_secs(1);
const WATCH_ERROR_BACKOFF_MIN: std::time::Duration = std::time::Duration::from_secs(5);
const WATCH_ERROR_BACKOFF_MAX: std::time::Duration = std::time::Duration::from_secs(300);

pub(super) struct WitnessWatchTask {
    _shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

/// Long-polls the witness service so remote changes trigger a sync round
/// promptly instead of waiting for the next background interval. The task
/// parks while no witness is configured and follows witness swaps via the
/// hook's change notifications; dropping the returned handle stops it.
pub(super) fn spawn_witness_watch(db: Arc<Db>, hook: Arc<E2eeSyncHook>) -> WitnessWatchTask {
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tauri::async_runtime::spawn(async move {
        let mut error_backoff = WATCH_ERROR_BACKOFF_MIN;
        let mut last_triggered: u64 = 0;
        let mut watched_workspace: Option<String> = None;
        loop {
            let witness_changed = hook.witness_changed.notified();
            tokio::pin!(witness_changed);
            witness_changed.as_mut().enable();
            let Some(witness) = hook.witness() else {
                tokio::select! {
                    _ = &mut shutdown_rx => return,
                    () = &mut witness_changed => {}
                }
                continue;
            };
            // A swapped witness (sign-out, account switch) starts over in a new
            // sequence space; carrying last_triggered across would suppress wakes
            // until the new head outgrows the old workspace's sequence.
            if watched_workspace.as_deref() != Some(witness.workspace_id()) {
                last_triggered = 0;
                watched_workspace = Some(witness.workspace_id().to_string());
            }

            tokio::select! {
                _ = &mut shutdown_rx => return,
                () = tokio::time::sleep(WATCH_POLL_PACING) => {}
            }

            let cursor =
                match anlg_db_app::e2ee_witness_cursor(db.pool(), witness.workspace_id()).await {
                    Ok(cursor) => cursor,
                    Err(error) => {
                        tracing::debug!(%error, "witness watch could not read the local cursor");
                        tokio::select! {
                            _ = &mut shutdown_rx => return,
                            () = tokio::time::sleep(error_backoff) => {}
                            () = &mut witness_changed => {}
                        }
                        error_backoff = (error_backoff * 2).min(WATCH_ERROR_BACKOFF_MAX);
                        continue;
                    }
                };
            let after = cursor.max(last_triggered);

            let cancellation = E2eeWitnessCancellation::default();
            let wait = witness.wait_for_remote_head(after, &cancellation);
            tokio::pin!(wait);
            let result = tokio::select! {
                _ = &mut shutdown_rx => return,
                () = &mut witness_changed => continue,
                result = &mut wait => result,
            };
            match result {
                Ok(Some(head)) => {
                    last_triggered = head;
                    error_backoff = WATCH_ERROR_BACKOFF_MIN;
                    db.cloudsync_request_sync();
                }
                Ok(None) => {
                    error_backoff = WATCH_ERROR_BACKOFF_MIN;
                }
                Err(error) => {
                    tracing::debug!(%error, "witness watch long-poll failed");
                    tokio::select! {
                        _ = &mut shutdown_rx => return,
                        () = tokio::time::sleep(error_backoff) => {}
                        () = &mut witness_changed => {}
                    }
                    error_backoff = (error_backoff * 2).min(WATCH_ERROR_BACKOFF_MAX);
                }
            }
        }
    });
    WitnessWatchTask {
        _shutdown_tx: shutdown_tx,
    }
}

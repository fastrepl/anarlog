//! Hosts the shared search-index projection worker
//! (`anlg_search_index::worker`) inside the Tauri app: waits for the plugin
//! and the database, then drains `search_index_dirty` as the DB changes.

use std::sync::Arc;
use std::time::Duration;

use anlg_search_index::worker::{DIRTY_DEBOUNCE, DrainOutcome, RETRY_INTERVAL};
use tauri::{AppHandle, Manager};
use tauri_plugin_tantivy::{IndexState, TantivyPluginExt};

pub fn spawn(app: AppHandle, db: Arc<anlg_db_core::Db>) {
    tauri::async_runtime::spawn(async move {
        run(app, db).await;
    });
}

async fn run(app: AppHandle, db: Arc<anlg_db_core::Db>) {
    let mut changes = db.change_notifier().subscribe();

    wait_for_tantivy(&app).await;
    if !wait_for_database_ready(&app).await {
        return;
    }

    let index = app.state::<IndexState>();
    let index = &index.collections;

    loop {
        match anlg_search_index::worker::initialize(index, db.pool()).await {
            Ok(DrainOutcome::Complete) => break,
            Ok(DrainOutcome::Deferred) => {
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
            Err(error) => {
                tracing::error!(%error, "failed to initialize search index projection");
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
        }
    }

    loop {
        match anlg_search_index::worker::drain_queue(index, db.pool()).await {
            Ok(DrainOutcome::Complete | DrainOutcome::Deferred) => {}
            Err(error) => {
                tracing::error!(%error, "failed to update search index projection");
            }
        }

        tokio::select! {
            change = changes.recv() => {
                match change {
                    Ok(change) if change.table == "search_index_dirty" => {
                        tokio::time::sleep(DIRTY_DEBOUNCE).await;
                        loop {
                            match changes.try_recv() {
                                Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
                                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
                            }
                        }
                    }
                    Ok(_) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = tokio::time::sleep(RETRY_INTERVAL) => {}
        }
    }
}

async fn wait_for_database_ready(app: &AppHandle) -> bool {
    loop {
        if let Some(runtime) = app.try_state::<tauri_plugin_db::ManagedState>() {
            return match runtime.wait_until_ready().await {
                Ok(()) => true,
                Err(error) => {
                    tracing::error!(%error, "search index waiting for database startup failed");
                    false
                }
            };
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_for_tantivy(app: &AppHandle) {
    loop {
        match app.tantivy().document_count(None).await {
            Ok(_) => return,
            Err(error) if error.is_collection_not_found() => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(error) => {
                tracing::warn!(%error, "search index is not ready");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

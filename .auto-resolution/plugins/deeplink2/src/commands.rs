use crate::pending_deep_link::PendingDeepLinkState;
use crate::pending_share_open::PendingShareOpenState;
use crate::server;
use crate::types::{DeepLink, ShareOpenRequest};

#[tauri::command]
#[specta::specta]
pub async fn start_callback_server<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scheme: String,
) -> Result<u16, String> {
    server::start(app, scheme).await
}

#[tauri::command]
#[specta::specta]
pub async fn stop_callback_server<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    server::stop(app).await
}

#[tauri::command]
#[specta::specta]
pub fn take_pending_deep_links(
    state: tauri::State<'_, PendingDeepLinkState>,
) -> Result<Vec<DeepLink>, String> {
    let deep_links = state
        .take_all()
        .map_err(|_| "pending deep-link queue unavailable".to_string())?;
    if !deep_links.is_empty() {
        tracing::info!(count = deep_links.len(), "pending_deep_links_drained");
    }
    Ok(deep_links)
}

#[tauri::command]
#[specta::specta]
pub fn list_pending_share_opens(
    state: tauri::State<'_, PendingShareOpenState>,
) -> Result<Vec<String>, String> {
    state
        .list()
        .map_err(|_| "pending shared-note queue unavailable".to_string())
}

#[tauri::command]
#[specta::specta]
pub fn take_pending_share_open(
    state: tauri::State<'_, PendingShareOpenState>,
    pending_id: String,
) -> Result<Option<ShareOpenRequest>, String> {
    state
        .take(&pending_id)
        .map_err(|_| "pending shared-note queue unavailable".to_string())
}

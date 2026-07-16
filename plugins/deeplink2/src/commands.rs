use crate::pending_share_open::PendingShareOpenState;
use crate::server;
use crate::types::ShareOpenRequest;

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

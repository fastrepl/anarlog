use hypr_hooks::{HookEvent, HookResult};

use crate::HooksPluginExt;

#[tauri::command]
#[specta::specta]
pub(crate) async fn run_event_hooks<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    event: HookEvent,
) -> Result<Vec<HookResult>, String> {
    app.hooks()
        .handle_event(event)
        .await
        .map_err(|e| e.to_string())
}

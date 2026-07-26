use crate::NotificationPluginExt;

#[tauri::command]
#[specta::specta]
pub(crate) async fn show_notification<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: hypr_notification::Notification,
) -> Result<(), String> {
    app.notification().show(v).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn clear_notifications<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    app.notification().clear().map_err(|e| e.to_string())
}

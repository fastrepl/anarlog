use crate::IconPluginExt;

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_dock_icon<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    name: String,
) -> Result<(), String> {
    app.icon().set_dock_icon(name).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn reset_dock_icon<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    app.icon().reset_dock_icon().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_icon<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    app.icon().get_icon().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_recording_indicator<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    show: bool,
) -> Result<(), String> {
    app.icon()
        .set_recording_indicator(show)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_notification_badge<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    count: Option<u8>,
) -> Result<(), String> {
    app.icon()
        .set_notification_badge(count)
        .map_err(|e| e.to_string())
}

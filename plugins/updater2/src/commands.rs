use crate::Updater2PluginExt;

#[tauri::command]
#[specta::specta]
pub(crate) async fn check<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    app.updater2().check().await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn download<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    version: String,
) -> Result<(), String> {
    app.updater2()
        .download(&version)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn install_and_relaunch<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    version: String,
) -> Result<(), String> {
    app.updater2()
        .install_and_relaunch(&version)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn is_downloaded<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    version: String,
) -> Result<bool, String> {
    Ok(app.updater2().has_cached_update(&version))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn set_automatic_updates_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    enabled: bool,
) -> Result<(), String> {
    app.updater2()
        .set_automatic_updates_enabled(enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn set_meeting_active<R: tauri::Runtime>(app: tauri::AppHandle<R>, active: bool) {
    app.updater2().set_meeting_active(active);
}

#[tauri::command]
#[specta::specta]
pub(crate) fn maybe_emit_updated<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    app.updater2().maybe_emit_updated();
}

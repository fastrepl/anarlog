use crate::{InstallResult, Updater2PluginExt};

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
pub(crate) async fn install<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    version: String,
) -> Result<InstallResult, String> {
    app.updater2()
        .install(&version)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn postinstall<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    result: InstallResult,
) -> Result<(), String> {
    app.updater2()
        .postinstall(result)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn maybe_emit_updated<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    app.updater2().maybe_emit_updated();
}

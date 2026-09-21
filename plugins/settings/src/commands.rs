use crate::SettingsPluginExt;

#[tauri::command]
#[specta::specta]
pub(crate) async fn global_base<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<String, String> {
    app.settings()
        .global_base()
        .map(|p| p.to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn settings_path<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<String, String> {
    app.settings()
        .settings_path()
        .map(|p| p.to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_base<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<String, String> {
    app.settings()
        .vault_base()
        .map(|p| p.to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn load<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<serde_json::Value, String> {
    app.settings().load().await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn save<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    settings: serde_json::Value,
) -> Result<(), String> {
    app.settings()
        .save(settings)
        .await
        .map_err(|e| e.to_string())
}

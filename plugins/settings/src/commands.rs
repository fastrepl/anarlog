use camino::Utf8PathBuf;

use crate::SettingsPluginExt;
use hypr_storage::ObsidianVault;

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
pub(crate) async fn copy_vault<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    new_path: String,
) -> Result<(), String> {
    app.settings()
        .copy_vault(Utf8PathBuf::from(&new_path))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn move_vault<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    new_path: String,
) -> Result<(), String> {
    app.settings()
        .move_vault(Utf8PathBuf::from(&new_path))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_vault_base<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    new_path: String,
) -> Result<(), String> {
    app.settings()
        .set_vault_base(Utf8PathBuf::from(&new_path))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn obsidian_vaults<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<ObsidianVault>, String> {
    app.settings().obsidian_vaults().map_err(|e| e.to_string())
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

use std::path::PathBuf;

use crate::Opener2PluginExt;

#[tauri::command]
#[specta::specta]
pub(crate) async fn open_url<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    url: String,
    open_with: Option<String>,
) -> Result<(), String> {
    app.opener2()
        .open_url(&url, open_with.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn open_path<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: String,
    open_with: Option<String>,
) -> Result<(), String> {
    app.opener2()
        .open_path(&path, open_with.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn reveal_item_in_dir<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
) -> Result<(), String> {
    app.opener2()
        .reveal_item_in_dir(&path)
        .map_err(|e| e.to_string())
}

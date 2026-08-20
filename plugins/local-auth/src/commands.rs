use tauri::Runtime;

use crate::LocalAuthPluginExt;

#[tauri::command]
#[specta::specta]
pub async fn available<R: Runtime>(app: tauri::AppHandle<R>) -> Result<bool, String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || handle.local_auth().available())
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn authenticate<R: Runtime>(
    app: tauri::AppHandle<R>,
    reason: String,
) -> Result<bool, String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || handle.local_auth().authenticate(&reason))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

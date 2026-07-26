use crate::AuthPluginExt;

#[tauri::command]
#[specta::specta]
pub(crate) fn decode_claims(token: String) -> Result<hypr_supabase_auth::Claims, String> {
    hypr_supabase_auth::Claims::decode_insecure(&token).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_account_info<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<hypr_template_support::AccountInfo>, String> {
    app.get_account_info().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_item<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    key: String,
) -> Result<Option<String>, String> {
    app.get_item(key).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_item<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    key: String,
    value: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.set_item(key, value))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn remove_item<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    key: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.remove_item(key))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn clear<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.clear_auth())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

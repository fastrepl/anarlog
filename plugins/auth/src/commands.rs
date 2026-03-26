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
pub(crate) fn set_item<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    key: String,
    value: String,
) -> Result<(), String> {
    app.set_item(key, value).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn remove_item<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    key: String,
) -> Result<(), String> {
    app.remove_item(key).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn clear<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.clear_auth().map_err(|e| e.to_string())
}

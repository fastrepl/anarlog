use crate::AuthPluginExt;

#[tauri::command]
#[specta::specta]
pub(crate) fn decode_claims(token: String) -> Result<anlg_supabase_auth::Claims, String> {
    anlg_supabase_auth::Claims::decode_insecure(&token).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_account_info<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<crate::AccountInfo>, String> {
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
    use tauri::Manager;
    app.state::<crate::refresh::RefreshGate>().clear();
    tauri::async_runtime::spawn_blocking(move || app.clear_auth())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn begin_refresh(
    gate: tauri::State<'_, crate::refresh::RefreshGate>,
    refresh_token: String,
) -> crate::refresh::RefreshPermit {
    gate.begin(&refresh_token, std::time::Instant::now())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn finish_refresh(
    gate: tauri::State<'_, crate::refresh::RefreshGate>,
    lease_id: u64,
    status: Option<u16>,
    retry_after_ms: Option<u64>,
) {
    gate.finish(lease_id, status, retry_after_ms, std::time::Instant::now());
}

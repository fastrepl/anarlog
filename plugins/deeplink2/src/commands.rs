use crate::server;

#[tauri::command]
#[specta::specta]
pub async fn start_callback_server<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scheme: String,
) -> Result<u16, String> {
    server::start(app, scheme).await
}

#[tauri::command]
#[specta::specta]
pub async fn stop_callback_server<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    server::stop(app).await
}

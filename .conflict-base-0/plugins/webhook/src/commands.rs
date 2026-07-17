use crate::WebhookPluginExt;

#[tauri::command]
#[specta::specta]
pub async fn todo<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<String, String> {
    app.webhook().todo()
}

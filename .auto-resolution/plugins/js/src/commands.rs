use crate::JsPluginExt;

#[tauri::command]
#[specta::specta]
pub(crate) async fn eval<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    code: String,
) -> Result<String, String> {
    app.js().eval(&code).await.map_err(|e| e.to_string())
}

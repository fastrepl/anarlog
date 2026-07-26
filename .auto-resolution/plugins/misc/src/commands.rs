use crate::MiscPluginExt;

#[tauri::command]
#[specta::specta]
pub async fn get_git_hash<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<String, String> {
    Ok(app.misc().get_git_hash())
}

#[tauri::command]
#[specta::specta]
pub async fn get_fingerprint<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<String, String> {
    Ok(app.misc().get_fingerprint())
}

#[tauri::command]
#[specta::specta]
pub async fn get_device_info<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    locale: Option<String>,
) -> Result<hypr_template_support::DeviceInfo, String> {
    Ok(app.misc().get_device_info(locale))
}

#[tauri::command]
#[specta::specta]
pub async fn opinionated_md_to_html<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    text: String,
) -> Result<String, String> {
    app.misc().opinionated_md_to_html(&text)
}

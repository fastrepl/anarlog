use crate::{Feature, FlagPluginExt};

#[tauri::command]
#[specta::specta]
pub(crate) async fn is_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    feature: Feature,
) -> Result<bool, String> {
    Ok(app.flag().is_enabled(feature).await)
}

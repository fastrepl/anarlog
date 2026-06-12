use crate::TrayPluginExt;

#[tauri::command]
#[specta::specta]
pub async fn set_tray_icon_visible(
    app: tauri::AppHandle<tauri::Wry>,
    visible: bool,
) -> Result<(), String> {
    app.tray().set_visible(visible).map_err(|e| e.to_string())?;
    Ok(())
}

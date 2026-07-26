use crate::{TrayPluginExt, schedule::TrayScheduleEvent};

#[tauri::command]
#[specta::specta]
pub async fn set_tray_icon_visible(
    app: tauri::AppHandle<tauri::Wry>,
    visible: bool,
) -> Result<(), String> {
    app.tray().set_visible(visible).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn set_tray_schedule(
    app: tauri::AppHandle<tauri::Wry>,
    events: Vec<TrayScheduleEvent>,
) -> Result<(), String> {
    app.tray()
        .set_schedule(events)
        .map_err(|error| error.to_string())
}

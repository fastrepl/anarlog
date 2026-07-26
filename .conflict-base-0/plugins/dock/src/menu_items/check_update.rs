use super::DockMenuItem;

pub struct DockCheckUpdate;

impl DockMenuItem for DockCheckUpdate {
    const SEPARATOR_BEFORE: bool = true;

    fn title(_app: &tauri::AppHandle<tauri::Wry>) -> String {
        "Check for Updates...".to_string()
    }

    fn handle(app: &tauri::AppHandle<tauri::Wry>) {
        tauri_plugin_tray::HyprMenuItem::TrayCheckUpdate.handle(app);
    }
}

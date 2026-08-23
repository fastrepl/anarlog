use super::DockMenuItem;

pub struct DockCheckUpdate;

impl DockMenuItem for DockCheckUpdate {
    const SEPARATOR_BEFORE: bool = true;

    fn enabled(_app: &tauri::AppHandle<tauri::Wry>) -> bool {
        tauri_plugin_tray::updates_enabled()
    }

    fn title(_app: &tauri::AppHandle<tauri::Wry>) -> String {
        "Check for Updates...".to_string()
    }

    fn handle(app: &tauri::AppHandle<tauri::Wry>) {
        if !tauri_plugin_tray::updates_enabled() {
            return;
        }

        tauri_plugin_tray::AnlgMenuItem::TrayCheckUpdate.handle(app);
    }
}

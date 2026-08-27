use super::DockMenuItem;

pub struct DockQuit;

impl DockMenuItem for DockQuit {
    fn title(app: &tauri::AppHandle<tauri::Wry>) -> String {
        format!("Quit {} Completely", app.package_info().name)
    }

    fn handle(app: &tauri::AppHandle<tauri::Wry>) {
        tauri_plugin_tray::quit_completely(app);
    }
}

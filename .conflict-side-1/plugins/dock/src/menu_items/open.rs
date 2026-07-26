use super::DockMenuItem;

pub struct DockOpen;

impl DockMenuItem for DockOpen {
    fn title(app: &tauri::AppHandle<tauri::Wry>) -> String {
        format!("Open {}", app.package_info().name)
    }

    fn handle(app: &tauri::AppHandle<tauri::Wry>) {
        tauri_plugin_tray::HyprMenuItem::TrayOpen.handle(app);
    }
}

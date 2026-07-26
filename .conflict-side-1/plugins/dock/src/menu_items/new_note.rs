use super::DockMenuItem;

pub struct DockNewNote;

impl DockMenuItem for DockNewNote {
    fn title(_app: &tauri::AppHandle<tauri::Wry>) -> String {
        "New Note".to_string()
    }

    fn handle(app: &tauri::AppHandle<tauri::Wry>) {
        tauri_plugin_tray::HyprMenuItem::AppNew.handle(app);
    }
}

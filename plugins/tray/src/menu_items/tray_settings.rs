use tauri::{
    AppHandle, Result,
    menu::{MenuItem, MenuItemKind},
};
use tauri_plugin_windows::{AppWindow, OpenTab, TabInput, WindowsPluginExt};
use tauri_specta::Event;

use super::MenuItemHandler;

pub struct TraySettings;

impl MenuItemHandler for TraySettings {
    const ID: &'static str = "hypr_tray_settings";

    fn build(app: &AppHandle<tauri::Wry>) -> Result<MenuItemKind<tauri::Wry>> {
        let item = MenuItem::with_id(app, Self::ID, "Settings", true, None::<&str>)?;
        Ok(MenuItemKind::MenuItem(item))
    }

    fn handle(app: &AppHandle<tauri::Wry>) {
        if app.windows().show(AppWindow::Main).is_ok() {
            let event = OpenTab {
                tab: TabInput::Settings { state: None },
            };
            if let Err(e) = event.emit(app) {
                tracing::warn!("failed_emit_open_settings_tab: {e}");
            }
        }
    }
}

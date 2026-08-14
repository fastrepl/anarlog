use tauri::{
    AppHandle, Result,
    menu::{MenuItem, MenuItemKind},
};
use tauri_plugin_windows::AppWindow;

use super::MenuItemHandler;

pub struct TrayHide;

impl MenuItemHandler for TrayHide {
    const ID: &'static str = "anlg_tray_hide";

    fn build(app: &AppHandle<tauri::Wry>) -> Result<MenuItemKind<tauri::Wry>> {
        let item = MenuItem::with_id(app, Self::ID, "Hide", true, None::<&str>)?;
        Ok(MenuItemKind::MenuItem(item))
    }

    fn handle(app: &AppHandle<tauri::Wry>) {
        let _ = AppWindow::Main.hide(app);
    }
}

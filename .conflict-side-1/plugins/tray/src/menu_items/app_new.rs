use tauri::{
    AppHandle, Result,
    menu::{MenuItem, MenuItemKind},
};

use super::MenuItemHandler;

pub struct AppNew;

impl MenuItemHandler for AppNew {
    const ID: &'static str = "hypr_app_new";

    fn build(app: &AppHandle<tauri::Wry>) -> Result<MenuItemKind<tauri::Wry>> {
        let item = MenuItem::with_id(app, Self::ID, "New Note", true, Some("CmdOrCtrl+N"))?;
        Ok(MenuItemKind::MenuItem(item))
    }

    fn handle(app: &AppHandle<tauri::Wry>) {
        use tauri_plugin_windows::{AppWindow, Navigate, WindowsPluginExt};
        if app.windows().show(AppWindow::Main).is_ok() {
            let _ = app.windows().emit_navigate(
                AppWindow::Main,
                Navigate {
                    path: "/app/new".to_string(),
                    search: None,
                },
            );
        }
    }
}

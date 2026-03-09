use tauri::{
    AppHandle, Result,
    menu::{MenuItem, MenuItemKind},
};

use super::MenuItemHandler;

pub struct TrayVersion;

impl TrayVersion {
    fn get_channel(app_name: &str) -> &'static str {
        match app_name {
            "Char" | "Hyprnote" => "stable",
            "Char Nightly" | "Hyprnote Nightly" => "nightly",
            "Char Staging" | "Hyprnote Staging" => "staging",
            _ => "dev",
        }
    }
}

impl MenuItemHandler for TrayVersion {
    const ID: &'static str = "hypr_tray_version";

    fn build(app: &AppHandle<tauri::Wry>) -> Result<MenuItemKind<tauri::Wry>> {
        let app_name = &app.package_info().name;
        let app_version = app.package_info().version.to_string();
        let channel = Self::get_channel(app_name);

        let text = format!("v{} ({})", app_version, channel);
        let item = MenuItem::with_id(app, Self::ID, text, false, None::<&str>)?;
        Ok(MenuItemKind::MenuItem(item))
    }

    fn handle(_app: &AppHandle<tauri::Wry>) {}
}

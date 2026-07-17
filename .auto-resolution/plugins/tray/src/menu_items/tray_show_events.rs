use tauri::{
    AppHandle, Result,
    menu::{CheckMenuItem, MenuItemKind},
};

use super::MenuItemHandler;
use crate::TrayPluginExt;

pub struct TrayShowEvents;

impl MenuItemHandler for TrayShowEvents {
    const ID: &'static str = "hypr_tray_show_events";

    fn build(app: &AppHandle<tauri::Wry>) -> Result<MenuItemKind<tauri::Wry>> {
        let item = CheckMenuItem::with_id(
            app,
            Self::ID,
            "Show events in menu bar",
            true,
            app.tray().shows_events(),
            None::<&str>,
        )?;
        Ok(MenuItemKind::Check(item))
    }

    fn handle(app: &AppHandle<tauri::Wry>) {
        let show = !app.tray().shows_events();
        if let Err(error) = app.tray().set_show_events(show) {
            tracing::warn!(%error, "failed to update tray event visibility");
        }
    }
}

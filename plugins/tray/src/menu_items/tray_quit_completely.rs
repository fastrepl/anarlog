use tauri::{
    AppHandle, Result,
    menu::{MenuItem, MenuItemKind},
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

use super::MenuItemHandler;

pub struct TrayQuitCompletely;

impl MenuItemHandler for TrayQuitCompletely {
    const ID: &'static str = "anlg_tray_quit_completely";

    fn build(app: &AppHandle<tauri::Wry>) -> Result<MenuItemKind<tauri::Wry>> {
        let item = MenuItem::with_id(app, Self::ID, "Quit Completely…", true, None::<&str>)?;
        Ok(MenuItemKind::MenuItem(item))
    }

    fn handle(app: &AppHandle<tauri::Wry>) {
        let app_name = app.package_info().name.clone();
        let app = app.clone();

        app.dialog()
            .message(format!("{} will stop running in the background.", app_name))
            .title(format!("Quit {} Completely?", app_name))
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Quit Completely".to_string(),
                "Cancel".to_string(),
            ))
            .show(move |confirmed| {
                if confirmed {
                    app.exit(0);
                }
            });
    }
}

use tauri::{
    AppHandle, Result,
    menu::{MenuId, MenuItem, MenuItemKind},
};
use tauri_plugin_windows::{AppWindow, OpenTab, TabInput, WindowsPluginExt};
use tauri_specta::Event;

const ID_PREFIX: &str = "hypr_tray_agenda_";

pub fn build_agenda_item(
    app: &AppHandle<tauri::Wry>,
    index: usize,
    text: &str,
) -> Result<MenuItemKind<tauri::Wry>> {
    let item = MenuItem::with_id(app, format!("{ID_PREFIX}{index}"), text, true, None::<&str>)?;
    Ok(MenuItemKind::MenuItem(item))
}

pub fn handle_agenda_menu_event(app: &AppHandle<tauri::Wry>, id: &MenuId) -> bool {
    if !id.0.starts_with(ID_PREFIX) {
        return false;
    }

    if app.windows().show(AppWindow::Main).is_ok() {
        let _ = OpenTab {
            tab: TabInput::Calendar,
        }
        .emit(app);
    }

    true
}

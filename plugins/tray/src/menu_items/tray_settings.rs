use tauri::{
    AppHandle, Result,
    menu::{MenuItem, MenuItemKind, Submenu},
};
use tauri_plugin_windows::{AppWindow, OpenTab, TabInput, WindowsPluginExt};
use tauri_specta::Event;

use super::MenuItemHandler;

pub struct TraySettings;

impl MenuItemHandler for TraySettings {
    const ID: &'static str = "hypr_tray_settings";

    fn build(app: &AppHandle<tauri::Wry>) -> Result<MenuItemKind<tauri::Wry>> {
        let submenu = {
            let submenu = Submenu::with_id(app, Self::ID, "Settings", true)?;

            let open_ai =
                MenuItem::with_id(app, TraySettingsAI::ID, "AI Settings", true, None::<&str>)?;

            let open_general = MenuItem::with_id(
                app,
                TraySettingsGeneral::ID,
                "App Settings",
                true,
                None::<&str>,
            )?;

            submenu.append_items(&[&open_ai, &open_general])?;
            submenu
        };

        Ok(MenuItemKind::Submenu(submenu))
    }

    fn handle(_app: &AppHandle<tauri::Wry>) {}
}

pub struct TraySettingsGeneral;

impl MenuItemHandler for TraySettingsGeneral {
    const ID: &'static str = "hypr_tray_settings_general";

    fn build(app: &AppHandle<tauri::Wry>) -> Result<MenuItemKind<tauri::Wry>> {
        let item = MenuItem::with_id(app, Self::ID, "App Settings", true, None::<&str>)?;
        Ok(MenuItemKind::MenuItem(item))
    }

    fn handle(app: &AppHandle<tauri::Wry>) {
        if app.windows().show(AppWindow::Main).is_ok() {
            let event = OpenTab {
                tab: TabInput::Settings,
            };
            if let Err(e) = event.emit(app) {
                tracing::warn!("failed_emit_open_settings_tab: {e}");
            }
        }
    }
}

pub struct TraySettingsAI;

impl MenuItemHandler for TraySettingsAI {
    const ID: &'static str = "hypr_tray_settings_ai";

    fn build(app: &AppHandle<tauri::Wry>) -> Result<MenuItemKind<tauri::Wry>> {
        let item = MenuItem::with_id(app, Self::ID, "AI Settings", true, None::<&str>)?;
        Ok(MenuItemKind::MenuItem(item))
    }

    fn handle(app: &AppHandle<tauri::Wry>) {
        if app.windows().show(AppWindow::Main).is_ok() {
            let event = OpenTab {
                tab: TabInput::Ai {
                    state: Some(Default::default()),
                },
            };
            if let Err(e) = event.emit(app) {
                tracing::warn!("failed_emit_open_ai_tab: {e}");
            }
        }
    }
}

use std::sync::{
    Mutex,
    atomic::{AtomicU8, Ordering},
};

use tauri::{
    AppHandle, Result,
    menu::{Menu, MenuItem, MenuItemKind, PredefinedMenuItem},
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_updater2::Updater2PluginExt;

use super::{MenuItemHandler, TrayOpen, TrayQuit, TrayStart, TrayVersion};

const STATE_CHECK_FOR_UPDATE: u8 = 0;
const STATE_DOWNLOADING: u8 = 1;
const STATE_RESTART_TO_APPLY: u8 = 2;

static UPDATE_STATE: AtomicU8 = AtomicU8::new(STATE_CHECK_FOR_UPDATE);
static PENDING_VERSION: Mutex<Option<String>> = Mutex::new(None);

pub struct TrayCheckUpdate;

impl TrayCheckUpdate {
    pub fn set_state(app: &AppHandle<tauri::Wry>, state: UpdateMenuState) -> Result<()> {
        let (text, enabled, state_value) = match &state {
            UpdateMenuState::CheckForUpdate => ("Check for Updates", true, STATE_CHECK_FOR_UPDATE),
            UpdateMenuState::Downloading => ("Downloading...", false, STATE_DOWNLOADING),
            UpdateMenuState::RestartToApply(_) => {
                ("Restart to Apply Update", true, STATE_RESTART_TO_APPLY)
            }
        };

        if let UpdateMenuState::RestartToApply(version) = state {
            *PENDING_VERSION.lock().unwrap() = Some(version);
        }

        UPDATE_STATE.store(state_value, Ordering::SeqCst);

        if let Some(menu) = app.menu()
            && let Some(item) = menu.get(Self::ID)
            && let MenuItemKind::MenuItem(menu_item) = item
        {
            menu_item.set_text(text)?;
            menu_item.set_enabled(enabled)?;
        }

        if let Some(tray) = app.tray_by_id("hypr-tray") {
            let check_update_item = MenuItem::with_id(app, Self::ID, text, enabled, None::<&str>)?;

            let menu = Menu::with_items(
                app,
                &[
                    &TrayOpen::build(app)?,
                    &TrayStart::build_with_disabled(app, false)?,
                    &PredefinedMenuItem::separator(app)?,
                    &MenuItemKind::MenuItem(check_update_item),
                    &PredefinedMenuItem::separator(app)?,
                    &TrayQuit::build(app)?,
                    &PredefinedMenuItem::separator(app)?,
                    &TrayVersion::build(app)?,
                ],
            )?;

            tray.set_menu(Some(menu))?;
        }

        Ok(())
    }

    fn get_state() -> u8 {
        UPDATE_STATE.load(Ordering::SeqCst)
    }

    fn pending_version() -> Option<String> {
        PENDING_VERSION.lock().unwrap().clone()
    }
}

#[derive(Debug, Clone)]
pub enum UpdateMenuState {
    CheckForUpdate,
    Downloading,
    RestartToApply(String),
}

impl MenuItemHandler for TrayCheckUpdate {
    const ID: &'static str = "hypr_tray_check_update";

    fn build(app: &AppHandle<tauri::Wry>) -> Result<MenuItemKind<tauri::Wry>> {
        let state = Self::get_state();

        let (text, enabled) = match state {
            STATE_DOWNLOADING => ("Downloading...", false),
            STATE_RESTART_TO_APPLY => ("Restart to Apply Update", true),
            _ => ("Check for Updates", true),
        };
        let item = MenuItem::with_id(app, Self::ID, text, enabled, None::<&str>)?;
        Ok(MenuItemKind::MenuItem(item))
    }

    fn handle(app: &AppHandle<tauri::Wry>) {
        let current_state = Self::get_state();

        if current_state == STATE_RESTART_TO_APPLY {
            if let Some(version) = Self::pending_version() {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    match app.updater2().install(&version).await {
                        Ok(()) => app.restart(),
                        Err(e) => {
                            app.dialog()
                                .message(format!("Failed to install update: {}", e))
                                .title("Update Failed")
                                .show(|_| {});
                        }
                    }
                });
            }
            return;
        }

        if current_state == STATE_DOWNLOADING {
            return;
        }

        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            match app.updater2().check().await {
                Ok(Some(version)) => {
                    let app_for_dialog = app.clone();
                    let version_for_download = version.clone();
                    app.dialog()
                        .message(format!("Update v{} is available!", version))
                        .title("Update Available")
                        .buttons(MessageDialogButtons::OkCancelCustom(
                            "Download".to_string(),
                            "Later".to_string(),
                        ))
                        .show(move |accepted| {
                            if accepted {
                                let app = app_for_dialog;
                                let version = version_for_download;
                                tauri::async_runtime::spawn(async move {
                                    if let Err(e) = app.updater2().download(&version).await {
                                        let _ = TrayCheckUpdate::set_state(
                                            &app,
                                            UpdateMenuState::CheckForUpdate,
                                        );
                                        app.dialog()
                                            .message(format!("Failed to download update: {}", e))
                                            .title("Update Failed")
                                            .show(|_| {});
                                    }
                                });
                            }
                        });
                }
                Ok(None) => {
                    app.dialog()
                        .message("There are currently no updates available.")
                        .title("Check for Updates")
                        .show(|_| {});
                }
                Err(e) => {
                    app.dialog()
                        .message(format!("Failed to check for updates: {}", e))
                        .title("Update Check Failed")
                        .show(|_| {});
                }
            }
        });
    }
}

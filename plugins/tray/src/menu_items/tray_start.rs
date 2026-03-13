use tauri::{
    AppHandle, Result,
    menu::{MenuItem, MenuItemKind},
};

use super::MenuItemHandler;

pub struct TrayStart;

impl MenuItemHandler for TrayStart {
    const ID: &'static str = "hypr_tray_start";

    fn build(app: &AppHandle<tauri::Wry>) -> Result<MenuItemKind<tauri::Wry>> {
        let item = MenuItem::with_id(app, Self::ID, "Start a new meeting", true, None::<&str>)?;
        Ok(MenuItemKind::MenuItem(item))
    }

    fn handle(app: &AppHandle<tauri::Wry>) {
        use tauri_plugin_windows::{AppWindow, OpenTab, SessionsState, TabInput, WindowsPluginExt};
        use tauri_specta::Event;

        if app.windows().show(AppWindow::Main).is_ok() {
            let event = OpenTab {
                tab: TabInput::Sessions {
                    id: "new".to_string(),
                    state: Some(SessionsState {
                        view: Default::default(),
                        auto_start: Some(true),
                    }),
                },
            };
            let _ = event.emit(app);
        }
    }
}

impl TrayStart {
    pub fn build_with_disabled(
        app: &AppHandle<tauri::Wry>,
        disabled: bool,
    ) -> Result<MenuItem<tauri::Wry>> {
        MenuItem::with_id(
            app,
            Self::ID,
            "Start a new meeting",
            !disabled,
            None::<&str>,
        )
    }
}

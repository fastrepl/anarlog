use tauri::{
    AppHandle, Result,
    menu::{MenuItem, MenuItemKind},
};

use super::MenuItemHandler;

pub struct HelpReportBug;

impl MenuItemHandler for HelpReportBug {
    const ID: &'static str = "hypr_help_report_bug";

    fn build(app: &AppHandle<tauri::Wry>) -> Result<MenuItemKind<tauri::Wry>> {
        let item = MenuItem::with_id(app, Self::ID, "Report Bug", true, None::<&str>)?;
        Ok(MenuItemKind::MenuItem(item))
    }

    fn handle(app: &AppHandle<tauri::Wry>) {
        use tauri_plugin_windows::{AppWindow, ChatState, OpenTab, TabInput, WindowsPluginExt};
        use tauri_specta::Event;

        if app.windows().show(AppWindow::Main).is_ok() {
            let event = OpenTab {
                tab: TabInput::ChatSupport {
                    state: Some(ChatState {
                        initial_message: Some("I'd like to report a bug.".to_string()),
                        ..Default::default()
                    }),
                },
            };
            let _ = event.emit(app);
        }
    }
}

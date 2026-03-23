use super::DockMenuItem;

pub struct DockNewNote;

impl DockMenuItem for DockNewNote {
    const TITLE: &'static str = "New Note";

    fn handle(app: &tauri::AppHandle<tauri::Wry>) {
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

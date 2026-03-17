mod commands;
mod errors;
mod events;
mod ext;
mod tab;
mod window;

pub use errors::*;
pub use events::*;
pub use ext::{Windows, WindowsPluginExt};
pub use tab::*;
pub use window::*;

const PLUGIN_NAME: &str = "windows";

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::Manager;
use tokio::sync::oneshot;

#[derive(Default)]
pub struct WindowReadyState {
    // Using tokio::sync::oneshot instead of std::sync::mpsc because:
    // std::mpsc::Receiver::recv_timeout blocks the thread, which can deadlock
    // when on_webview_ready callback needs the blocked thread to signal.
    // tokio::oneshot allows async await with timeout that yields instead of blocking.
    pending: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

impl WindowReadyState {
    pub fn register(&self, label: String) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(label, tx);
        rx
    }

    pub fn signal(&self, label: &str) {
        if let Some(tx) = self.pending.lock().unwrap().remove(label) {
            let _ = tx.send(());
        }
    }
}

fn make_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .plugin_name(PLUGIN_NAME)
        .events(tauri_specta::collect_events![
            events::Navigate,
            events::WindowDestroyed,
            events::OpenTab,
            events::VisibilityEvent,
        ])
        .commands(tauri_specta::collect_commands![
            commands::window_show,
            commands::window_destroy,
            commands::window_navigate,
            commands::window_emit_navigate,
            commands::window_is_exists,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            {
                let ready_state = WindowReadyState::default();
                app.manage(ready_state);
            }

            Ok(())
        })
        .on_webview_ready(|webview| {
            if let Some(state) = webview.app_handle().try_state::<WindowReadyState>() {
                state.signal(webview.label());
            }
        })
        .on_event(move |app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                use tauri_plugin_window_state::{AppHandleExt, StateFlags};
                let _ = app.save_window_state(StateFlags::SIZE);
            }
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }

    #[test]
    fn test_version() {
        let version = tauri_plugin_os::version()
            .to_string()
            .split('.')
            .next()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        println!("version: {}", version);
    }
}

mod commands;
mod error;
mod events;
mod ext;
mod global;
mod handler;

pub use error::*;
pub use events::*;
pub use ext::*;

use handler::Handler;
use tauri::Manager;

const PLUGIN_NAME: &str = "shortcut";

fn make_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::register_hotkey::<tauri::Wry>,
            commands::unregister_hotkey::<tauri::Wry>,
            global::configure,
            global::validate,
            global::set_active,
        ])
        .events(tauri_specta::collect_events![ShortcutEvent])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);
            let handler = Handler::new();
            app.manage(handler);
            app.manage(global::GlobalState::default());
            Ok(())
        })
        .build()
}

// Call from the application setup hook, after Tauri releases its plugin initialization lock.
pub fn initialize_global_shortcuts(app: &tauri::AppHandle) {
    if !global::uses_portal()
        && let Err(error) = app.plugin(tauri_plugin_global_shortcut::Builder::new().build())
    {
        tracing::warn!(%error, "Global shortcuts are unavailable");
    }
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
}

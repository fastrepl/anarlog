mod commands;
mod error;
mod events;
mod ext;
mod handler;

pub use error::*;
pub use events::*;
pub use ext::*;

use handler::Handler;
use tauri::Manager;

const PLUGIN_NAME: &str = "dictation";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::show::<tauri::Wry>,
            commands::hide::<tauri::Wry>,
            commands::set_phase::<tauri::Wry>,
            commands::update_amplitude::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            app.manage(Handler::new());
            setup_shortcut_bridge(app);
            Ok(())
        })
        .build()
}

fn setup_shortcut_bridge(app: &tauri::AppHandle) {
    use ext::DictationPluginExt;
    use tauri_plugin_shortcut::ShortcutEvent;
    use tauri_specta::Event;

    let handle = app.clone();
    ShortcutEvent::listen(app, move |event| {
        let d = handle.dictation();
        match event.payload {
            ShortcutEvent::Pressed => {
                let _ = d.set_phase(Phase::Recording);
                let _ = d.show();
            }
            ShortcutEvent::Released => {
                let _ = d.set_phase(Phase::Processing);
                let _ = d.hide();
            }
            ShortcutEvent::Cancelled | ShortcutEvent::Discarded => {
                let _ = d.hide();
            }
        }
    });
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder::<tauri::Wry>()
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

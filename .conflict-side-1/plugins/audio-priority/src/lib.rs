use tauri::Manager;
use tauri_plugin_settings::SettingsPluginExt;

mod commands;
mod error;
mod ext;
mod priority;
mod state;

pub use error::{Error, Result};
pub use ext::*;
pub use priority::*;
pub use state::*;

pub use hypr_audio_device::{
    AudioDevice, AudioDeviceBackend, AudioDirection, DeviceId, TransportType, backend,
};

const PLUGIN_NAME: &str = "audio-priority";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::list_input_devices::<tauri::Wry>,
            commands::list_output_devices::<tauri::Wry>,
            commands::set_default_input_device::<tauri::Wry>,
            commands::set_default_output_device::<tauri::Wry>,
            commands::get_input_priorities::<tauri::Wry>,
            commands::get_output_priorities::<tauri::Wry>,
            commands::save_input_priorities::<tauri::Wry>,
            commands::save_output_priorities::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app, _api| {
            let base = app.settings().global_base().unwrap();
            let state = AudioPriorityState::new(base.into_std_path_buf());
            assert!(app.manage(state));
            Ok(())
        })
        .build()
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

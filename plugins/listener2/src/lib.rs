use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

mod commands;
mod ext;

pub use ext::*;

pub use hypr_listener2_core::*;

const PLUGIN_NAME: &str = "listener2";

pub type SharedState = Arc<Mutex<State>>;

pub struct State {
    pub app: tauri::AppHandle,
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::run_batch::<tauri::Wry>,
            commands::run_denoise::<tauri::Wry>,
            commands::parse_subtitle::<tauri::Wry>,
            commands::export_to_vtt::<tauri::Wry>,
            commands::is_supported_languages_batch::<tauri::Wry>,
            commands::suggest_providers_for_languages_batch::<tauri::Wry>,
            commands::list_documented_language_codes_batch::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![BatchEvent, DenoiseEvent])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            let app_handle = app.app_handle().clone();
            let state: SharedState = Arc::new(Mutex::new(State { app: app_handle }));
            app.manage(state);

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

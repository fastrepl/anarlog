use std::sync::Arc;

use tauri::Manager;

mod commands;
mod error;
mod events;
mod ext;
pub mod runtime;

pub use error::{Error, Result};
pub use events::*;
pub use ext::*;

const PLUGIN_NAME: &str = "activity-capture";

pub type ManagedState<R> = Arc<runtime::ActivityCaptureRuntime<R>>;

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::capabilities::<tauri::Wry>,
            commands::snapshot::<tauri::Wry>,
            commands::start::<tauri::Wry>,
            commands::stop::<tauri::Wry>,
            commands::is_running::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![ActivityCapturePluginEvent])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);
            app.manage(Arc::new(runtime::ActivityCaptureRuntime::new(
                app.app_handle().clone(),
            )));
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

    fn create_app<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::App<R> {
        builder
            .plugin(init())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
    }

    #[test]
    fn test_plugin_init() {
        let _app = create_app(tauri::test::mock_builder());
    }
}

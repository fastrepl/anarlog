use std::sync::Arc;

use ractor::Actor;
use tauri::Manager;

mod commands;
mod error;
mod ext;
mod runtime;

pub use error::{DegradedError, Error, Result};
pub use ext::*;
pub use hypr_listener_core::*;

use hypr_listener_core::actors::{RootActor, RootArgs};
use runtime::TauriRuntime;

const PLUGIN_NAME: &str = "listener";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::list_microphone_devices::<tauri::Wry>,
            commands::get_current_microphone_device::<tauri::Wry>,
            commands::get_mic_muted::<tauri::Wry>,
            commands::set_mic_muted::<tauri::Wry>,
            commands::start_session::<tauri::Wry>,
            commands::stop_session::<tauri::Wry>,
            commands::get_state::<tauri::Wry>,
            commands::is_supported_languages_live::<tauri::Wry>,
            commands::suggest_providers_for_languages_live::<tauri::Wry>,
            commands::list_documented_language_codes_live::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![
            SessionLifecycleEvent,
            SessionProgressEvent,
            SessionErrorEvent,
            SessionDataEvent
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            let app_handle = app.app_handle().clone();

            let runtime = Arc::new(TauriRuntime {
                app: app_handle.clone(),
            });

            tauri::async_runtime::spawn(async move {
                Actor::spawn(Some(RootActor::name()), RootActor, RootArgs { runtime })
                    .await
                    .map(|_| tracing::info!("root_actor_spawned"))
                    .map_err(|e| tracing::error!(?e, "failed_to_spawn_root_actor"))
            });

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

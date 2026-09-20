mod commands;
mod error;
mod events;
mod ext;
#[cfg(target_os = "macos")]
mod startup_migration;
mod store;

pub use error::{Error, Result};
pub use events::*;
pub use ext::*;
pub(crate) use store::*;

const PLUGIN_NAME: &str = "updater2";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::check::<tauri::Wry>,
            commands::download::<tauri::Wry>,
            commands::install_and_relaunch::<tauri::Wry>,
            commands::is_downloaded::<tauri::Wry>,
            commands::set_automatic_updates_enabled::<tauri::Wry>,
            commands::set_meeting_active::<tauri::Wry>,
            commands::maybe_emit_updated::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![
            events::UpdateAvailableEvent,
            events::UpdateDownloadingEvent,
            events::UpdateDownloadProgressEvent,
            events::UpdateDownloadFailedEvent,
            events::UpdateReadyEvent,
            events::UpdatedEvent,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            #[cfg(target_os = "macos")]
            match startup_migration::maybe_schedule_legacy_bundle_rename_on_launch(app) {
                Ok(true) => std::process::exit(0),
                Ok(false) => {}
                Err(err) => tracing::error!("failed to schedule legacy bundle rename: {}", err),
            }

            // [fork] No background auto-check/auto-install: this build has no
            // release channel of its own, and desktop.anarlog.so serves
            // upstream's official binary, which would silently replace this
            // fork (and its privacy/unlock patches) on the next tick.

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

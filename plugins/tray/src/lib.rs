mod commands;
mod ext;
mod menu_items;
mod schedule;
mod tray_icon;

use std::sync::atomic::{AtomicBool, Ordering};

pub use ext::*;
pub use menu_items::{AnlgMenuItem, UpdateMenuState, handle_agenda_menu_event, quit_completely};

const PLUGIN_NAME: &str = "anlg-tray";
static UPDATES_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn init(enable_updates: bool) -> tauri::plugin::TauriPlugin<tauri::Wry> {
    UPDATES_ENABLED.store(enable_updates, Ordering::SeqCst);
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::<tauri::Wry>::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app, _api| {
            if updates_enabled() {
                setup_update_listeners(app);
            }
            Ok(())
        })
        .build()
}

pub fn updates_enabled() -> bool {
    UPDATES_ENABLED.load(Ordering::SeqCst)
}

fn setup_update_listeners(app: &tauri::AppHandle) {
    use ext::TrayPluginExt;
    use tauri_specta::Event;

    let handle = app.clone();
    tauri_plugin_updater2::UpdateAvailableEvent::listen(app, move |_event| {
        let _ = handle.tray().set_update_available(true);
    });

    let handle = app.clone();
    tauri_plugin_updater2::UpdateDownloadingEvent::listen(app, move |_event| {
        let _ = menu_items::TrayCheckUpdate::set_state(&handle, UpdateMenuState::Downloading);
        let _ = handle.tray().set_update_available(true);
    });

    let handle = app.clone();
    tauri_plugin_updater2::UpdateReadyEvent::listen(app, move |event| {
        let _ = menu_items::TrayCheckUpdate::set_state(
            &handle,
            UpdateMenuState::RestartToApply(event.payload.version.clone()),
        );
        let _ = handle.tray().set_update_available(true);
    });

    let handle = app.clone();
    tauri_plugin_updater2::UpdateDownloadFailedEvent::listen(app, move |_event| {
        let _ = menu_items::TrayCheckUpdate::set_state(&handle, UpdateMenuState::CheckForUpdate);
        let _ = handle.tray().set_update_available(false);
    });

    let handle = app.clone();
    tauri_plugin_updater2::UpdatedEvent::listen(app, move |_event| {
        let _ = menu_items::TrayCheckUpdate::set_state(&handle, UpdateMenuState::CheckForUpdate);
        let _ = handle.tray().set_update_available(false);
    });
}

fn make_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::set_tray_icon_visible,
            commands::set_tray_schedule,
            commands::set_tray_recording_title,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
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

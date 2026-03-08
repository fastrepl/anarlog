mod ext;
mod menu_items;
mod tray_icon;

pub use ext::*;
pub use menu_items::{HyprMenuItem, UpdateMenuState};

const PLUGIN_NAME: &str = "tray";

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::<tauri::Wry>::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app, _api| {
            setup_update_listeners(app);
            Ok(())
        })
        .build()
}

fn setup_update_listeners(app: &tauri::AppHandle) {
    use tauri_specta::Event;

    let handle = app.clone();
    tauri_plugin_updater2::UpdateDownloadingEvent::listen(app, move |_event| {
        let _ = menu_items::TrayCheckUpdate::set_state(&handle, UpdateMenuState::Downloading);
    });

    let handle = app.clone();
    tauri_plugin_updater2::UpdateReadyEvent::listen(app, move |event| {
        let _ = menu_items::TrayCheckUpdate::set_state(
            &handle,
            UpdateMenuState::RestartToApply(event.payload.version.clone()),
        );
    });

    let handle = app.clone();
    tauri_plugin_updater2::UpdateDownloadFailedEvent::listen(app, move |_event| {
        let _ = menu_items::TrayCheckUpdate::set_state(&handle, UpdateMenuState::CheckForUpdate);
    });

    let handle = app.clone();
    tauri_plugin_updater2::UpdatedEvent::listen(app, move |_event| {
        let _ = menu_items::TrayCheckUpdate::set_state(&handle, UpdateMenuState::CheckForUpdate);
    });
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
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

mod commands;
mod contacts;
mod error;
mod events;
mod runtime;
mod storage;

pub use anlg_calendar::ProviderConnectionIds;
pub use error::Error;
pub use events::*;

pub(crate) struct PluginConfig {
    pub api_base_url: String,
}

const PLUGIN_NAME: &str = "calendar";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::available_providers,
            commands::is_provider_enabled::<tauri::Wry>,
            commands::list_connection_ids::<tauri::Wry>,
            commands::list_calendars::<tauri::Wry>,
            commands::list_events::<tauri::Wry>,
            commands::open_calendar::<tauri::Wry>,
            commands::create_event::<tauri::Wry>,
            commands::apply_calendar_inventory::<tauri::Wry>,
            commands::tombstone_calendar_connection::<tauri::Wry>,
            commands::set_calendar_enabled::<tauri::Wry>,
            commands::sync_calendar_connection_events::<tauri::Wry>,
            commands::update_ignored_calendar_item::<tauri::Wry>,
            commands::create_human::<tauri::Wry>,
            commands::create_organization::<tauri::Wry>,
            commands::save_personal_contact::<tauri::Wry>,
            commands::update_human::<tauri::Wry>,
            commands::update_organization::<tauri::Wry>,
            commands::soft_delete_contact::<tauri::Wry>,
            commands::update_contact_avatar::<tauri::Wry>,
            commands::update_human_contact_summary::<tauri::Wry>,
            commands::toggle_contact_pin::<tauri::Wry>,
            commands::reorder_pinned_contacts::<tauri::Wry>,
            commands::merge_humans::<tauri::Wry>,
            commands::apply_contact_enhancement::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![CalendarChangedEvent])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();
    let api_base_url = get_api_base_url();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            anlg_calendar::start(runtime::TauriCalendarRuntime(app.app_handle().clone()));

            use tauri::Manager;
            app.manage(PluginConfig { api_base_url });
            Ok(())
        })
        .build()
}

fn get_api_base_url() -> String {
    #[cfg(not(debug_assertions))]
    {
        env!("VITE_API_URL").to_string()
    }

    #[cfg(debug_assertions)]
    {
        option_env!("VITE_API_URL")
            .unwrap_or("http://localhost:3001")
            .to_string()
    }
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

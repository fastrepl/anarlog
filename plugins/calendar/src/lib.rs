mod auth;
mod commands;
mod error;
mod events;
mod runtime;
mod sync_source;
mod sync_store;

pub use error::Error;
pub use events::*;
pub use hypr_calendar::ProviderConnectionIds;

pub(crate) struct PluginConfig {
    pub api_base_url: String,
}

pub(crate) struct CalendarSyncState(pub hypr_calendar_sync::CalendarSyncHandle);

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
            commands::parse_meeting_link,
            commands::request_calendar_sync::<tauri::Wry>,
            commands::get_calendar_sync_status::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![
            CalendarChangedEvent,
            CalendarSyncEvent
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();
    let api_base_url = get_api_base_url();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            use tauri::Manager;
            app.manage(PluginConfig { api_base_url });

            let handle = hypr_calendar_sync::start(
                sync_source::PluginCalendarSyncSource::new(app.app_handle().clone()),
                runtime::TauriCalendarSyncRuntime(app.app_handle().clone()),
                hypr_calendar_sync::Config::every_minute(),
                |task| {
                    tauri::async_runtime::spawn(task);
                },
            );
            app.manage(CalendarSyncState(handle));

            hypr_calendar::start(runtime::TauriCalendarRuntime(app.app_handle().clone()));
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

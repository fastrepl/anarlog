mod commands;
mod error;
mod events;
mod fetch;

pub use error::Error;
pub use events::*;

pub(crate) struct PluginConfig {
    pub api_base_url: String,
}

const PLUGIN_NAME: &str = "todo";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::authorization_status,
            commands::request_full_access,
            commands::list_todo_lists,
            commands::fetch_todos,
            commands::create_todo,
            commands::complete_todo,
            commands::delete_todo,
            commands::linear_list_teams::<tauri::Wry>,
            commands::linear_list_tickets::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![TodoChangedEvent])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();
    let api_base_url = get_api_base_url();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            #[cfg(target_os = "macos")]
            {
                use tauri_specta::Event;

                let app_handle = app.app_handle().clone();
                hypr_apple_todo::setup_change_notification(move || {
                    let _ = TodoChangedEvent.emit(&app_handle);
                });
            }

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

use tauri::Manager;

const MAX_CONCURRENT_FIRE_AND_FORGET_EVENTS: usize = 32;

mod commands;
mod error;
mod ext;
mod session;
mod store;

pub use error::{Error, Result};
pub use ext::*;
use session::*;
use store::*;

pub use anlg_analytics::*;

pub struct ManagedState {
    client: anlg_analytics::AnalyticsClient,
    fire_and_forget_slots: std::sync::Arc<tokio::sync::Semaphore>,
    groups: std::sync::Mutex<std::collections::HashMap<String, String>>,
    session: std::sync::Mutex<SessionTracker>,
}

impl ManagedState {
    fn new(client: anlg_analytics::AnalyticsClient) -> Self {
        Self {
            client,
            fire_and_forget_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(
                MAX_CONCURRENT_FIRE_AND_FORGET_EVENTS,
            )),
            groups: std::sync::Mutex::new(std::collections::HashMap::new()),
            session: std::sync::Mutex::new(SessionTracker::new(std::time::SystemTime::now())),
        }
    }
}

const PLUGIN_NAME: &str = "analytics";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::event_fire_and_forget::<tauri::Wry>,
            commands::event::<tauri::Wry>,
            commands::set_properties::<tauri::Wry>,
            commands::set_disabled::<tauri::Wry>,
            commands::is_disabled::<tauri::Wry>,
            commands::identify::<tauri::Wry>,
            commands::clear_groups::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app, _api| {
            let posthog_key = {
                #[cfg(not(debug_assertions))]
                {
                    let v = env!("POSTHOG_API_KEY");
                    assert!(v.starts_with("phc_"));
                    Some(v)
                }

                #[cfg(debug_assertions)]
                {
                    option_env!("POSTHOG_API_KEY")
                }
            };

            let client = {
                let mut builder = anlg_analytics::AnalyticsClientBuilder::default();
                if let Some(key) = posthog_key {
                    builder = builder.with_posthog(key);
                }

                builder.build()
            };

            assert!(app.manage(ManagedState::new(client)));
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

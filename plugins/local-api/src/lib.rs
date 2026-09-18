mod commands;

pub use anlg_local_api_core::dispatch::{
    EVENT_MEETING_COMPLETED, EVENT_NOTE_ENHANCED, KNOWN_EVENTS,
};
pub use anlg_local_api_core::types::*;

const PLUGIN_NAME: &str = "local-api";

fn make_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .plugin_name(PLUGIN_NAME)
        .events(tauri_specta::collect_events![])
        .commands(tauri_specta::collect_commands![
            commands::list_webhooks::<tauri::Wry>,
            commands::create_webhook::<tauri::Wry>,
            commands::delete_webhook::<tauri::Wry>,
            commands::set_webhook_active::<tauri::Wry>,
            commands::test_webhook::<tauri::Wry>,
            commands::dispatch_event::<tauri::Wry>,
            commands::export_meeting_markdown::<tauri::Wry>,
            commands::get_cloud_snapshot::<tauri::Wry>,
            commands::list_cloud_snapshot_ids::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);
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

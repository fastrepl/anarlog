mod commands;
mod error;
mod ext;
mod feature;

pub use error::*;
pub use ext::*;
pub use feature::*;

pub type ManagedState = hypr_analytics::AnalyticsClient;

const PLUGIN_NAME: &str = "flag";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::is_enabled::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .build()
}

#[cfg(test)]
mod test {
    use super::*;
    use tauri::Manager;

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
        let mut ctx = tauri::test::mock_context(tauri::test::noop_assets());
        ctx.config_mut().identifier = "com.hyprnote.dev".to_string();
        ctx.config_mut().version = Some("1.0.0".to_string());

        builder
            .plugin(init())
            .setup(|app| {
                let client = hypr_analytics::AnalyticsClientBuilder::default().build();
                app.manage(client);
                Ok(())
            })
            .build(ctx)
            .unwrap()
    }

    #[tokio::test]
    async fn test_flag() {
        let app = create_app(tauri::test::mock_builder());
        let _result = app.flag().is_enabled(Feature::Chat).await;
    }
}

use tauri::Manager;

mod commands;
mod error;
mod ext;
mod store;

pub use error::{Error, Result};
pub use ext::*;
use store::*;

pub use hypr_analytics::*;

pub type ManagedState = hypr_analytics::AnalyticsClient;

const PLUGIN_NAME: &str = "analytics";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::event::<tauri::Wry>,
            commands::set_properties::<tauri::Wry>,
            commands::set_disabled::<tauri::Wry>,
            commands::is_disabled::<tauri::Wry>,
            commands::identify::<tauri::Wry>,
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

            let outlit_key = option_env!("OUTLIT_PUBLIC_KEY");

            let client = {
                let mut builder = hypr_analytics::AnalyticsClientBuilder::default();
                if let Some(key) = posthog_key {
                    builder = builder.with_posthog(key);
                }
                if let Some(key) = outlit_key {
                    builder = builder.with_outlit(key);
                }

                builder.build()
            };

            assert!(app.manage(client));
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

    fn create_app<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::App<R> {
        let mut ctx = tauri::test::mock_context(tauri::test::noop_assets());
        ctx.config_mut().identifier = "com.hyprnote.dev".to_string();
        ctx.config_mut().version = Some("0.0.1".to_string());

        builder.plugin(init()).build(ctx).unwrap()
    }

    #[tokio::test]
    async fn test_analytics() {
        let app = create_app(tauri::test::mock_builder());
        let result = app
            .analytics()
            .event(hypr_analytics::AnalyticsPayload::builder("test_event").build())
            .await;
        assert!(result.is_ok());

        {
            use tauri_plugin_misc::MiscPluginExt;
            let git_hash = app.misc().get_git_hash();
            println!("git_hash: {}", git_hash);
        }

        {
            let version = app.config().version.clone();
            println!("version: {}", version.unwrap_or_default());
        }

        {
            let bundle_id = app.config().identifier.clone();
            println!("bundle_id: {}", bundle_id);
        }
    }
}

use tauri::Wry;

mod commands;
mod error;
mod ext;
mod sources;
mod types;

pub use error::*;
pub use ext::*;
pub use sources::{AsIsData, all_sources, import_all, list_available_sources};
pub use types::*;

const PLUGIN_NAME: &str = "importer";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::list_available_sources::<Wry>,
            commands::run_import::<Wry>,
            commands::run_import_dry::<Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|_app, _api| Ok(()))
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

    #[ignore]
    #[tokio::test]
    async fn test_run_import_dry() {
        let app = create_app(tauri::test::mock_builder());
        let importer = app.importer();

        println!(
            "{:?}",
            importer
                .run_import_dry(ImportSourceKind::HyprnoteV0Stable)
                .await
                .unwrap()
        );

        println!(
            "{:?}",
            importer
                .run_import_dry(ImportSourceKind::HyprnoteV0Nightly)
                .await
                .unwrap()
        );
    }
}

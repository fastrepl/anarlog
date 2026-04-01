mod commands;
mod ext;
mod migrate;

pub use ext::*;
pub use hypr_supabase_auth::client::{Error, Result};
use tauri::Manager;

const PLUGIN_NAME: &str = "auth";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::decode_claims,
            commands::get_item::<tauri::Wry>,
            commands::set_item::<tauri::Wry>,
            commands::remove_item::<tauri::Wry>,
            commands::clear::<tauri::Wry>,
            commands::get_account_info::<tauri::Wry>,
        ])
        .typ::<hypr_supabase_auth::Claims>()
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app, _api| {
            let auth_path = migrate::auth_path(app)?;
            app.manage(hypr_supabase_auth::client::store::AuthStore::load(
                auth_path,
            ));
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
        builder
            .plugin(tauri_plugin_settings::init())
            .plugin(init())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
    }

    #[test]
    fn test_auth() {
        let app = create_app(tauri::test::mock_builder());

        let _ = app.set_item("test_key".to_string(), "test_value".to_string());
        let _ = app.get_item("test_key".to_string());
    }
}

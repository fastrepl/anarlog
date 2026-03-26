mod commands;
mod ext;

pub use ext::*;
pub use hypr_supabase_auth::client::{Error, Result};

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
            use tauri::Manager;
            use tauri_plugin_settings::SettingsPluginExt;

            let base = app.settings().global_base().map_err(|e| e.to_string())?;
            let auth_path = std::path::Path::new(base.as_str()).join("auth.json");

            if !auth_path.exists() {
                migrate_from_store_json(
                    &std::path::Path::new(base.as_str()).join("store.json"),
                    &auth_path,
                )
                .ok();
            }

            app.manage(hypr_supabase_auth::client::store::AuthStore::load(
                auth_path,
            ));
            Ok(())
        })
        .build()
}

fn migrate_from_store_json(
    store_json_path: &std::path::Path,
    auth_path: &std::path::Path,
) -> std::io::Result<()> {
    if !store_json_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(store_json_path)?;
    let mut store: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&content).map_err(invalid_data)?;

    let auth_str = match store
        .remove(PLUGIN_NAME)
        .and_then(|v| v.as_str().map(|s| s.to_owned()))
    {
        Some(s) => s,
        None => return Ok(()),
    };

    let _: std::collections::HashMap<String, String> =
        serde_json::from_str(&auth_str).map_err(invalid_data)?;

    hypr_storage::fs::atomic_write(auth_path, &auth_str)?;
    hypr_storage::fs::atomic_write(
        store_json_path,
        &serde_json::to_string(&store).map_err(invalid_data)?,
    )?;

    Ok(())
}

fn invalid_data(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
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

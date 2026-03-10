mod error;
mod ext;
pub mod migrations;
pub mod version;

pub use error::{Error, Result};
pub use ext::*;
pub use version::*;

const PLUGIN_NAME: &str = "fs-db";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app, _api| {
            use tauri_plugin_settings::SettingsPluginExt;

            let base_dir = match app.settings().resolve_startup_vault_base() {
                Ok(dir) => dir,
                Err(_) => {
                    return Ok(());
                }
            };

            let app_version = app.config().version.as_ref().map_or_else(
                || hypr_version::Version::new(0, 0, 0),
                |v| {
                    v.parse::<hypr_version::Version>()
                        .expect("version must be semver")
                },
            );

            std::thread::spawn({
                let base_dir = base_dir.clone();
                let app_version = app_version.clone();
                move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("failed to create migration runtime");
                    rt.block_on(migrations::run(&base_dir, &app_version))
                }
            })
            .join()
            .expect("migration thread panicked")?;

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

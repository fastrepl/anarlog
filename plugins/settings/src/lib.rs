use tauri::Manager;

mod commands;
mod error;
mod ext;
mod state;

pub use error::{Error, Result};
pub use ext::*;
pub use hypr_storage::ObsidianVault;
pub use state::*;

const PLUGIN_NAME: &str = "settings";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::settings_path::<tauri::Wry>,
            commands::global_base::<tauri::Wry>,
            commands::vault_base::<tauri::Wry>,
            commands::copy_vault::<tauri::Wry>,
            commands::move_vault::<tauri::Wry>,
            commands::set_vault_base::<tauri::Wry>,
            commands::load::<tauri::Wry>,
            commands::save::<tauri::Wry>,
            commands::obsidian_vaults::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            let startup_vault_base = app.settings().resolve_startup_vault_base().unwrap();
            let snapshot = state::StartupSnapshot::new(startup_vault_base);
            assert!(app.manage(snapshot));
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

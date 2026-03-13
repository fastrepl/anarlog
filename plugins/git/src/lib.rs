mod commands;
mod error;
mod ext;
mod operations;
pub mod types;

pub use error::{Error, Result};
pub use ext::*;

const PLUGIN_NAME: &str = "git";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::is_repo::<tauri::Wry>,
            commands::init::<tauri::Wry>,
            commands::status::<tauri::Wry>,
            commands::add::<tauri::Wry>,
            commands::reset::<tauri::Wry>,
            commands::commit::<tauri::Wry>,
            commands::log::<tauri::Wry>,
            commands::add_remote::<tauri::Wry>,
            commands::list_remotes::<tauri::Wry>,
            commands::fetch::<tauri::Wry>,
            commands::push::<tauri::Wry>,
            commands::pull::<tauri::Wry>,
            commands::check_conflicts::<tauri::Wry>,
            commands::abort_merge::<tauri::Wry>,
            commands::get_current_branch::<tauri::Wry>,
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

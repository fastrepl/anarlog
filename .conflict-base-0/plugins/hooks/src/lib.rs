mod commands;
mod config;
mod error;
mod ext;

#[cfg(test)]
mod docs;

pub use error::*;
pub use ext::*;

const PLUGIN_NAME: &str = "hooks";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::run_event_hooks::<tauri::Wry>,
        ])
        .typ::<hypr_hooks::HooksConfig>()
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
    fn export() {
        export_types();
        export_docs();
    }

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

    fn export_docs() {
        let source_code = std::fs::read_to_string("./js/bindings.gen.ts").unwrap();
        let hooks = docs::parse_hooks(&source_code).unwrap();
        assert!(!hooks.is_empty());

        let output_dir = std::path::Path::new("../../apps/web/content/hooks");
        std::fs::create_dir_all(output_dir).unwrap();

        for hook in &hooks {
            let filepath = output_dir.join(hook.doc_path());
            let content = hook.doc_render();
            std::fs::write(&filepath, content).unwrap();
        }
    }
}

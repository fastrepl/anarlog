use tauri::Wry;

mod commands;
mod ext;
mod generated_summary;
mod session_content;
mod transaction_utils;

pub use anlg_template_app::Template;
pub use ext::TemplatePluginExt;

const PLUGIN_NAME: &str = "template";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::summary_length_policy,
            commands::dominant_language,
            commands::prepare_generated_summary,
            commands::compose_generated_summary,
            commands::save_generated_summary::<Wry>,
            commands::save_generated_title::<Wry>,
            commands::apply_session_content_corrections::<Wry>,
            commands::render::<Wry>,
            commands::render_custom::<Wry>,
            commands::get_template_source::<Wry>,
        ])
        .typ::<anlg_gbnf::Grammar>()
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|_app, _api| {
            let _ = anlg_template_app_legacy::get_environment();
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

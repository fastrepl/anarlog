mod commands;
mod ducking;
mod engine;
mod insertion;
mod lease;
mod preview;
mod recovery;

use tauri::Manager;

const PLUGIN_NAME: &str = "dictation";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::capture_target,
            commands::begin_dictation::<tauri::Wry>,
            commands::attach_dictation_stream::<tauri::Wry>,
            commands::finish_dictation::<tauri::Wry>,
            commands::complete_dictation::<tauri::Wry>,
            commands::release_dictation::<tauri::Wry>,
            commands::cancel_dictation::<tauri::Wry>,
            commands::retire_dictations::<tauri::Wry>,
            commands::list_dictation_recovery::<tauri::Wry>,
            commands::prepare_dictation_recovery_audio::<tauri::Wry>,
            commands::save_dictation_recovery_text::<tauri::Wry>,
            commands::claim_dictation_recovery::<tauri::Wry>,
            commands::discard_dictation_recovery::<tauri::Wry>,
            commands::fit_dictation_text,
            commands::prepare_dictation_refinement,
            commands::resolve_dictation_refinement,
            commands::suggest_dictation_terms,
            commands::rank_dictation_terms,
            commands::record_dictation_terms,
            commands::read_dictation_context,
            commands::insert_dictation::<tauri::Wry>,
            commands::read_dictation_kept,
            commands::release_dictation_target,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            let recovery = app
                .path()
                .app_data_dir()
                .ok()
                .map(|directory| directory.join("dictation-recovery"));
            app.manage(engine::Engine::new(
                recovery,
                insertion::output_ducker(),
                engine::Limits::default(),
            ));
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Ok(store) = app.state::<engine::Engine>().recovery() {
                    store.prune(engine::now_ms());
                }
            });
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

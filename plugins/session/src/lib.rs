use tauri::Wry;

mod attachments;
mod commands;
mod conflicts;
mod creation;
mod deletion;
mod folder_catalog;
mod folder_materials;
mod move_contents;
mod participants;
mod proposals;
mod transaction_utils;

#[cfg(test)]
mod attachment_folder_tests;
#[cfg(test)]
mod tests;

const PLUGIN_NAME: &str = "session";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::create_session::<Wry>,
            commands::create_session_for_event::<Wry>,
            commands::soft_delete_session::<Wry>,
            commands::restore_deleted_session::<Wry>,
            commands::add_session_participant::<Wry>,
            commands::remove_session_participant::<Wry>,
            commands::persist_chat_session_proposal::<Wry>,
            commands::set_session_proposal_status::<Wry>,
            commands::resolve_session_conflicts::<Wry>,
            commands::resolve_session_conflict::<Wry>,
            commands::move_session_contents::<Wry>,
            commands::catalog_note_attachment::<Wry>,
            commands::catalog_session_audio::<Wry>,
            commands::mark_session_audio_transcription_complete::<Wry>,
            commands::set_attachment_cloud_sync_enabled::<Wry>,
            commands::tombstone_session_audio::<Wry>,
            commands::mark_session_audio_absent::<Wry>,
            commands::catalog_folder_material::<Wry>,
            commands::tombstone_folder_material::<Wry>,
            commands::ensure_folder_catalog::<Wry>,
            commands::rename_folder_catalog::<Wry>,
            commands::delete_folder_catalog::<Wry>,
            commands::update_folder_instructions::<Wry>,
            commands::update_folder_workspace::<Wry>,
            commands::update_folder_icon::<Wry>,
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

const COMMANDS: &[&str] = &[
    "create_session",
    "create_session_for_event",
    "soft_delete_session",
    "restore_deleted_session",
    "add_session_participant",
    "remove_session_participant",
    "persist_chat_session_proposal",
    "set_session_proposal_status",
    "resolve_session_conflicts",
    "resolve_session_conflict",
    "move_session_contents",
    "catalog_note_attachment",
    "catalog_session_audio",
    "mark_session_audio_transcription_complete",
    "set_attachment_cloud_sync_enabled",
    "tombstone_session_audio",
    "mark_session_audio_absent",
    "catalog_folder_material",
    "tombstone_folder_material",
    "ensure_folder_catalog",
    "rename_folder_catalog",
    "delete_folder_catalog",
    "update_folder_instructions",
    "update_folder_workspace",
    "update_folder_icon",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

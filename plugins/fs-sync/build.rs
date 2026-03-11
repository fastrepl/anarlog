const COMMANDS: &[&str] = &[
    "deserialize",
    "write_json_batch",
    "write_document_batch",
    "read_document_batch",
    "list_folders",
    "move_session",
    "create_folder",
    "rename_folder",
    "delete_folder",
    "cleanup_orphan",
    "audio_exist",
    "audio_delete",
    "audio_import",
    "audio_path",
    "session_dir",
    "load_session_content",
    "delete_session_folder",
    "scan_and_read",
    "chat_dir",
    "entity_dir",
    "attachment_save",
    "attachment_list",
    "attachment_remove",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

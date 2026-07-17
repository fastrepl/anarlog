const COMMANDS: &[&str] = &[
    "describe_upload",
    "prepare_upload",
    "read_upload_range",
    "read_attachment_range",
    "download_and_restore",
    "cleanup_transfer_cache",
    "download_shared_attachment",
    "shared_attachment_path",
    "remove_shared_attachment",
    "clear_shared_attachment_scope",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

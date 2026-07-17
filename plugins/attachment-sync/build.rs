const COMMANDS: &[&str] = &[
    "describe_upload",
    "prepare_upload",
    "read_upload_range",
    "begin_shared_upload_operation",
    "cancel_shared_upload_operation",
    "prepare_shared_upload",
    "read_shared_upload_range",
    "validate_shared_upload",
    "cleanup_shared_upload",
    "prepare_delete_guard",
    "commit_delete_guard",
    "reconcile_delete_guards",
    "begin_attachment_download",
    "cancel_attachment_download",
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

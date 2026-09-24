const COMMANDS: &[&str] = &[
    "begin_connected_import",
    "cancel_connected_import",
    "complete_connected_import",
    "sync_connected_import",
    "list_crm_providers",
    "begin_crm_connection",
    "cancel_crm_connection",
    "complete_crm_connection",
    "verify_crm_connection",
    "lookup_crm_contacts",
    "read_text_files",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

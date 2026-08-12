const COMMANDS: &[&str] = &[
    "begin_connected_import",
    "cancel_connected_import",
    "complete_connected_import",
    "sync_connected_import",
    "list_available_sources",
    "run_import",
    "run_import_dry",
    "read_text_files",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

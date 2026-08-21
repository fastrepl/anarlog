const COMMANDS: &[&str] = &[
    "begin_connected_import",
    "cancel_connected_import",
    "complete_connected_import",
    "sync_connected_import",
    "read_text_files",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

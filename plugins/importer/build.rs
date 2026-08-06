const COMMANDS: &[&str] = &[
    "list_available_sources",
    "run_import",
    "run_import_dry",
    "read_text_files",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

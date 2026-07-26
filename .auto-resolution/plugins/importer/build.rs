const COMMANDS: &[&str] = &["list_available_sources", "run_import", "run_import_dry"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

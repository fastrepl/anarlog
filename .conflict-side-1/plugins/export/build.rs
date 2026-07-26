const COMMANDS: &[&str] = &["export", "export_text"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

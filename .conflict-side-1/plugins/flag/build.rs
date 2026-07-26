const COMMANDS: &[&str] = &["is_enabled"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

const COMMANDS: &[&str] = &["available", "authenticate"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

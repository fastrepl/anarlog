const COMMANDS: &[&str] = &["register_hotkey", "unregister_hotkey"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

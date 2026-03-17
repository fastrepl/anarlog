const COMMANDS: &[&str] = &["start_callback_server", "stop_callback_server"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

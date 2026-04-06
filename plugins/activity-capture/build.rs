const COMMANDS: &[&str] = &["capabilities", "snapshot", "start", "stop", "is_running"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

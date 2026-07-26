const COMMANDS: &[&str] = &["show_notification", "clear_notifications"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

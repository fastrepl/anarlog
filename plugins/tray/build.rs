const COMMANDS: &[&str] = &["set_tray_icon_visible"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

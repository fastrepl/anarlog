const COMMANDS: &[&str] = &["set_tray_icon_visible", "set_tray_schedule"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

const COMMANDS: &[&str] = &[
    "set_tray_icon_visible",
    "set_tray_schedule",
    "set_tray_recording_title",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

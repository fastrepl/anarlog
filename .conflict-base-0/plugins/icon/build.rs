const COMMANDS: &[&str] = &[
    "set_dock_icon",
    "reset_dock_icon",
    "get_icon",
    "set_recording_indicator",
    "set_notification_badge",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

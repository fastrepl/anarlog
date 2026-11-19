const COMMANDS: &[&str] = &[
    "get_state",
    "set_enabled",
    "set_auto_switch",
    "set_background",
    "add_extension",
    "allow_camera",
    "open_extension_settings",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

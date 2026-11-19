const COMMANDS: &[&str] = &[
    "register_hotkey",
    "unregister_hotkey",
    "configure",
    "configure_gestures",
    "validate",
    "set_active",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

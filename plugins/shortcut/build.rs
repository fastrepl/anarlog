const COMMANDS: &[&str] = &[
    "register_hotkey",
    "unregister_hotkey",
    "configure",
    "set_active",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

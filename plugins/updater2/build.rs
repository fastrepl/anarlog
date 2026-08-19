const COMMANDS: &[&str] = &[
    "check",
    "download",
    "install_and_relaunch",
    "is_downloaded",
    "maybe_emit_updated",
    "set_automatic_updates_enabled",
    "set_meeting_active",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

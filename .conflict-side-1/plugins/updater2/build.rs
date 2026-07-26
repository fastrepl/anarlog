const COMMANDS: &[&str] = &[
    "check",
    "download",
    "install",
    "is_downloaded",
    "maybe_emit_updated",
    "postinstall",
    "set_automatic_updates_enabled",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

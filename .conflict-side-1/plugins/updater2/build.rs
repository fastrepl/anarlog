const COMMANDS: &[&str] = &[
    "check",
    "download",
    "install",
    "is_downloaded",
    "postinstall",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

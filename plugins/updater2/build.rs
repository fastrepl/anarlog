const COMMANDS: &[&str] = &["check", "download", "install", "postinstall"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

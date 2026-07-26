const COMMANDS: &[&str] = &["sanitize"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

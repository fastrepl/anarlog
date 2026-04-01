const COMMANDS: &[&str] = &["relay_result"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

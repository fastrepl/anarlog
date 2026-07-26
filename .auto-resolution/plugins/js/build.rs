const COMMANDS: &[&str] = &["eval"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

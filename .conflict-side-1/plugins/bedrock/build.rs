const COMMANDS: &[&str] = &["list_foundation_models"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

const COMMANDS: &[&str] = &["run_event_hooks"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

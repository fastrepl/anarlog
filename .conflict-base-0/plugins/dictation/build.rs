const COMMANDS: &[&str] = &["show", "hide", "set_phase", "update_amplitude"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

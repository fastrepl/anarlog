const COMMANDS: &[&str] = &["set_fake_window_bounds", "remove_fake_window"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

const COMMANDS: &[&str] = &["play", "stop", "set_volume"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

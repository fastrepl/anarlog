const COMMANDS: &[&str] = &["health_check", "install_cli", "uninstall_cli"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

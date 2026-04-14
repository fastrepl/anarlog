const COMMANDS: &[&str] = &["execute", "execute_proxy", "subscribe", "unsubscribe"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

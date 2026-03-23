const COMMANDS: &[&str] = &["subscribe", "unsubscribe", "execute"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

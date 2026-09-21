const COMMANDS: &[&str] = &["global_base", "vault_base", "path", "load", "save"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

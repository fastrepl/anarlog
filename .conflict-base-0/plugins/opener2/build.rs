const COMMANDS: &[&str] = &["open_url", "open_path", "reveal_item_in_dir"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

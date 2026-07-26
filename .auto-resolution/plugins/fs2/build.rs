const COMMANDS: &[&str] = &["read_text_file", "remove"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

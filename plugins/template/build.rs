const COMMANDS: &[&str] = &["render", "render_custom", "render_support"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

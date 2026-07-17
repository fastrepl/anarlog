const COMMANDS: &[&str] = &[
    "render",
    "render_custom",
    "render_support",
    "get_template_source",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

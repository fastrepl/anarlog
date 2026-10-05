const COMMANDS: &[&str] = &[
    "summary_length_policy",
    "dominant_language",
    "prepare_generated_summary",
    "compose_generated_summary",
    "save_generated_summary",
    "save_generated_title",
    "apply_session_content_corrections",
    "render",
    "render_custom",
    "render_support",
    "get_template_source",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

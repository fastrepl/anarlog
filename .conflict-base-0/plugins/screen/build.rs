const COMMANDS: &[&str] = &[
    "capture_frontmost_window_context",
    "capture_target_window_context",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

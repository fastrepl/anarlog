const COMMANDS: &[&str] = &[
    "window_show",
    "window_destroy",
    "window_navigate",
    "window_emit_navigate",
    "window_is_exists",
    "window_set_frame_animated",
    "window_save_frame",
    "window_restore_frame_animated",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

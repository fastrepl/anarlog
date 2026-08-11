const COMMANDS: &[&str] = &[
    "open_permission",
    "check_permission",
    "request_permission",
    "reset_permission",
    "permission_guidance",
    "close_permission_assistant",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

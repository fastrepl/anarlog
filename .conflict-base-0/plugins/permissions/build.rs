const COMMANDS: &[&str] = &[
    "open_permission",
    "check_permission",
    "request_permission",
    "reset_permission",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

const COMMANDS: &[&str] = &[
    "is_available",
    "join_meeting",
    "leave_meeting",
    "get_status",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

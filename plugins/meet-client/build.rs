const COMMANDS: &[&str] = &[
    "is_available",
    "join_meeting",
    "leave_meeting",
    "get_status",
    "set_selectors",
    "report_observation",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

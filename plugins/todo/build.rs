const COMMANDS: &[&str] = &[
    "authorization_status",
    "request_full_access",
    "list_todo_lists",
    "fetch_todos",
    "create_todo",
    "complete_todo",
    "delete_todo",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}

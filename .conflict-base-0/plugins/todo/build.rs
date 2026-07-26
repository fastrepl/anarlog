const COMMANDS: &[&str] = &[
    "authorization_status",
    "request_full_access",
    "list_todo_lists",
    "fetch_todos",
    "read_path",
    "create_todo",
    "complete_todo",
    "delete_todo",
    "linear_list_teams",
    "linear_list_tickets",
    "github_issue_state",
    "github_issue_detail",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

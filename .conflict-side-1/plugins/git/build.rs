const COMMANDS: &[&str] = &[
    "is_repo",
    "init",
    "status",
    "add",
    "reset",
    "commit",
    "log",
    "add_remote",
    "list_remotes",
    "fetch",
    "push",
    "pull",
    "check_conflicts",
    "abort_merge",
    "get_current_branch",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

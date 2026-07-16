const COMMANDS: &[&str] = &[
    "list_pending_share_opens",
    "start_callback_server",
    "stop_callback_server",
    "take_pending_share_open",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

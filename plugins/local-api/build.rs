const COMMANDS: &[&str] = &[
    "list_webhooks",
    "create_webhook",
    "delete_webhook",
    "set_webhook_active",
    "test_webhook",
    "dispatch_event",
    "export_meeting_markdown",
    "get_cloud_snapshot",
    "list_cloud_snapshot_ids",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

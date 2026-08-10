const COMMANDS: &[&str] = &[
    "get_status",
    "set_enabled",
    "list_api_keys",
    "create_api_key",
    "revoke_api_key",
    "list_webhooks",
    "create_webhook",
    "delete_webhook",
    "test_webhook",
    "dispatch_event",
    "export_meeting_markdown",
    "get_cloud_snapshot",
    "list_cloud_snapshot_ids",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

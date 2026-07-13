const COMMANDS: &[&str] = &[
    "execute",
    "execute_proxy",
    "execute_transaction",
    "get_meeting",
    "get_meeting_transcript",
    "get_recurring_meeting_history",
    "get_legacy_cleanup_status",
    "get_legacy_import_report",
    "list_meetings",
    "cleanup_legacy_files",
    "run_legacy_import",
    "subscribe",
    "unsubscribe",
    "configure_cloudsync",
    "claim_cloudsync_account",
    "configure_cloudsync_token",
    "start_cloudsync",
    "stop_cloudsync",
    "suspend_cloudsync",
    "get_cloudsync_status",
    "sync_cloudsync_now",
    "logout_cloudsync",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

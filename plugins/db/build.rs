const COMMANDS: &[&str] = &[
    "execute",
    "execute_proxy",
    "execute_transaction",
    "get_legacy_cleanup_status",
    "get_legacy_import_report",
    "cleanup_legacy_files",
    "run_legacy_import",
    "subscribe",
    "unsubscribe",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

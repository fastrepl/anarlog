const COMMANDS: &[&str] = &[
    "available_providers",
    "is_provider_enabled",
    "list_connection_ids",
    "list_calendars",
    "list_events",
    "open_calendar",
    "create_event",
    "parse_meeting_link",
    "request_calendar_sync",
    "get_calendar_sync_status",
    "set_calendar_enabled",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

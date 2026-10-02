const COMMANDS: &[&str] = &[
    "available_providers",
    "is_provider_enabled",
    "list_connection_ids",
    "list_calendars",
    "list_events",
    "open_calendar",
    "create_event",
    "apply_calendar_inventory",
    "tombstone_calendar_connection",
    "set_calendar_enabled",
    "sync_calendar_connection_events",
    "update_ignored_calendar_item",
    "create_human",
    "create_organization",
    "save_personal_contact",
    "update_human",
    "update_organization",
    "soft_delete_contact",
    "update_contact_avatar",
    "update_human_contact_summary",
    "toggle_contact_pin",
    "reorder_pinned_contacts",
    "merge_humans",
    "apply_contact_enhancement",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

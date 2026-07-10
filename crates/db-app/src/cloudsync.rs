use std::sync::LazyLock;

use hypr_db_core::CloudsyncTableSpec;

static CLOUDSYNC_TABLE_REGISTRY: LazyLock<Vec<CloudsyncTableSpec>> = LazyLock::new(|| {
    [
        "action_items",
        "calendars",
        "chat_groups",
        "chat_messages",
        "daily_notes",
        "entity_mentions",
        "events",
        "humans",
        "organizations",
        "session_attachments",
        "session_documents",
        "session_participants",
        "session_tags",
        "sessions",
        "tags",
        "templates",
        "transcripts",
    ]
    .into_iter()
    .map(|table_name| CloudsyncTableSpec {
        table_name: table_name.to_string(),
        crdt_algo: None,
        force_init: None,
        enabled: false,
    })
    .collect()
});

pub fn cloudsync_table_registry() -> &'static [CloudsyncTableSpec] {
    CLOUDSYNC_TABLE_REGISTRY.as_slice()
}

pub fn cloudsync_alter_guard_required(table_name: &str) -> bool {
    cloudsync_table_registry()
        .iter()
        .any(|table| table.enabled && table.table_name == table_name)
}

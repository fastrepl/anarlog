use std::sync::LazyLock;

use hypr_db_core::CloudsyncTableSpec;

static CLOUDSYNC_TABLE_REGISTRY: LazyLock<Vec<CloudsyncTableSpec>> = LazyLock::new(|| {
    [
        ("action_items", true),
        ("calendars", false),
        ("chat_groups", false),
        ("chat_messages", false),
        ("daily_notes", false),
        ("entity_mentions", false),
        ("events", false),
        ("humans", true),
        ("organizations", true),
        ("session_attachments", true),
        ("session_documents", true),
        ("session_participants", true),
        ("session_tags", false),
        ("sessions", true),
        ("tags", false),
        ("templates", false),
        ("transcripts", true),
    ]
    .into_iter()
    .map(|(table_name, enabled)| CloudsyncTableSpec {
        table_name: table_name.to_string(),
        crdt_algo: None,
        init_flags: None,
        enabled,
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

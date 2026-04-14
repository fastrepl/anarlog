use std::sync::LazyLock;

use hypr_db_core2::CloudsyncTableSpec;

static CLOUDSYNC_TABLE_REGISTRY: LazyLock<Vec<CloudsyncTableSpec>> = LazyLock::new(|| {
    vec![CloudsyncTableSpec {
        table_name: "templates".to_string(),
        crdt_algo: None,
        force_init: None,
        enabled: false,
    }]
});

pub fn cloudsync_table_registry() -> &'static [CloudsyncTableSpec] {
    CLOUDSYNC_TABLE_REGISTRY.as_slice()
}

pub fn cloudsync_alter_guard_required(table_name: &str) -> bool {
    cloudsync_table_registry()
        .iter()
        .any(|table| table.enabled && table.table_name == table_name)
}

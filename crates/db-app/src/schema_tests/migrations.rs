use super::*;

#[tokio::test]
async fn schema_declares_legacy_migrations_and_cloudsync_registry() {
    let db = test_db().await;

    let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();

    assert!(tables.contains(&"_sqlx_migrations".to_string()));
    assert!(tables.contains(&"templates".to_string()));
    assert!(tables.contains(&"sessions".to_string()));
    assert!(tables.contains(&"migration_import_runs".to_string()));
}

#[tokio::test]
async fn migrations_apply_cleanly() {
    let db = test_db().await;

    let tables: Vec<String> = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(db.pool())
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.0)
        .collect();

    assert_eq!(
        tables,
        vec![
            "_sqlx_migrations",
            "action_items",
            "api_keys",
            "app_settings",
            "attachment_local_state",
            "attachment_transfer_jobs",
            "calendars",
            "chat_groups",
            "chat_messages",
            "cloudsync_session_evictions",
            "cloudsync_writable_workspaces",
            "daily_notes",
            "e2ee_apply_guard",
            "e2ee_dirty_rows",
            "e2ee_local_device",
            "e2ee_local_state",
            "e2ee_records",
            "e2ee_witness_pending",
            "e2ee_witness_records",
            "e2ee_witness_state",
            "entity_mentions",
            "events",
            "humans",
            "migration_import_items",
            "migration_import_runs",
            "migration_import_targets",
            "organizations",
            "search_index_dirty",
            "search_index_state",
            "session_attachments",
            "session_documents",
            "session_participants",
            "session_share_sync_state",
            "session_tags",
            "sessions",
            "shared_session_attachment_cache",
            "shared_session_cache",
            "storage_migration_state",
            "tags",
            "templates",
            "transcripts",
            "webhook_endpoints",
            "workspace_memberships",
            "workspaces",
        ]
    );
}

#[tokio::test]
async fn personal_workspace_migration_preserves_existing_session_workspace_ids() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(
        &db,
        anlg_db_migrate::DbSchema {
            steps: migration_steps_before("20260716120000_personal_workspaces"),
            validate_cloudsync_table: cloudsync_alter_guard_required,
        },
    )
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'user-1', 'user-1', 'Existing note')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    anlg_db_migrate::migrate(&db, schema()).await.unwrap();
    sqlx::query(
        "INSERT INTO workspaces (id, owner_user_id, name)
             VALUES ('user-1', 'user-1', 'Personal')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id, role)
             VALUES ('membership-1', 'user-1', 'user-1', 'owner')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let session_workspace_id: String =
        sqlx::query_scalar("SELECT workspace_id FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let workspace: (String, String) =
        sqlx::query_as("SELECT id, kind FROM workspaces WHERE id = 'user-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let membership_role: String =
        sqlx::query_scalar("SELECT role FROM workspace_memberships WHERE id = 'membership-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();

    assert_eq!(session_workspace_id, "user-1");
    assert_eq!(workspace, ("user-1".to_string(), "personal".to_string()));
    assert_eq!(membership_role, "owner");

    let duplicate = sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id)
             VALUES ('membership-2', 'user-1', 'user-1')",
    )
    .execute(db.pool())
    .await;
    assert!(duplicate.is_err());
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn workspace_tables_can_be_initialized_by_cloudsync() {
    let db = Db::connect_memory().await.unwrap();
    prepare_schema(&db).await.unwrap();

    for table_name in ["workspaces", "workspace_memberships"] {
        db.cloudsync_init(table_name, None, None).await.unwrap();
        assert!(
            anlg_db_core::cloudsync_is_enabled_on(db.pool(), table_name)
                .await
                .unwrap()
        );
    }
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn search_index_migrations_apply_before_cloudsync_initialization() {
    let db = Db::connect_memory().await.unwrap();

    prepare_schema(&db).await.unwrap();

    let trigger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'trigger' AND name LIKE 'search_index_%'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(trigger_count, 15);

    initialize_enabled_cloudsync_tables(&db).await;

    sqlx::query("INSERT INTO sessions (id, title) VALUES ('session-1', 'Planning')")
        .execute(db.pool())
        .await
        .unwrap();
    let generation: i64 = sqlx::query_scalar(
        "SELECT generation FROM search_index_dirty
             WHERE entity_type = 'session' AND entity_id = 'session-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(generation, 1);
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn search_index_migrations_apply_to_initialized_cloudsync_tables() {
    let db = Db::connect_memory().await.unwrap();
    anlg_db_migrate::migrate(
        &db,
        anlg_db_migrate::DbSchema {
            steps: migration_steps_before("20260714120000_search_index_queue"),
            validate_cloudsync_table: cloudsync_alter_guard_required,
        },
    )
    .await
    .unwrap();
    for table_name in E2EE_DOMAIN_TABLES {
        db.cloudsync_init(table_name, None, None).await.unwrap();
    }

    prepare_schema(&db).await.unwrap();
    initialize_enabled_cloudsync_tables(&db).await;

    sqlx::query("INSERT INTO sessions (id, title) VALUES ('session-1', 'Planning')")
        .execute(db.pool())
        .await
        .unwrap();
    let generation: i64 = sqlx::query_scalar(
        "SELECT generation FROM search_index_dirty
             WHERE entity_type = 'session' AND entity_id = 'session-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(generation, 1);

    for table in cloudsync_table_registry()
        .iter()
        .filter(|table| table.enabled)
    {
        assert!(
            anlg_db_core::cloudsync_is_enabled_on(db.pool(), &table.table_name)
                .await
                .unwrap()
        );
    }
}

#[tokio::test]
async fn migration_repairs_empty_titles_from_summary_headings() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(
        &db,
        anlg_db_migrate::DbSchema {
            steps: migration_steps_before("20260713164500_repair_empty_session_titles"),
            validate_cloudsync_table: cloudsync_alter_guard_required,
        },
    )
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO sessions (id, title)
             VALUES ('json', ''), ('markdown', '   '), ('generic', ''), ('existing', 'Keep Me')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
            "INSERT INTO session_documents
             (id, session_id, kind, body_format, body, sort_order)
             VALUES
             ('json-summary', 'json', 'summary', 'prosemirror_json',
              '{\"type\":\"doc\",\"content\":[{\"type\":\"heading\",\"attrs\":{\"level\":1},\"content\":[{\"type\":\"text\",\"text\":\"Transcript Test \"},{\"type\":\"text\",\"text\":\"Utterances\"}]}]}', 0),
             ('markdown-summary', 'markdown', 'summary', 'markdown',
              char(10) || '# Markdown Title' || char(10) || char(10) || 'Details', 0),
             ('generic-summary', 'generic', 'summary', 'markdown', '# Summary' || char(10) || 'Details', 0),
             ('existing-summary', 'existing', 'summary', 'markdown', '# Replacement' || char(10) || 'Details', 0)",
        )
        .execute(db.pool())
        .await
        .unwrap();

    anlg_db_migrate::migrate(&db, schema()).await.unwrap();

    let titles =
        sqlx::query_as::<_, (String, String)>("SELECT id, title FROM sessions ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap()
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>();

    assert_eq!(titles["json"], "Transcript Test Utterances");
    assert_eq!(titles["markdown"], "Markdown Title");
    assert_eq!(titles["generic"], "");
    assert_eq!(titles["existing"], "Keep Me");
}

#[tokio::test]
async fn repair_migration_recreates_missing_templates_table() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(
        &db,
        anlg_db_migrate::DbSchema {
            steps: &APP_MIGRATION_STEPS[..3],
            validate_cloudsync_table: cloudsync_alter_guard_required,
        },
    )
    .await
    .unwrap();

    sqlx::query("DROP TABLE templates")
        .execute(db.pool())
        .await
        .unwrap();

    anlg_db_migrate::migrate(&db, schema()).await.unwrap();

    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM templates")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(row_count, 0);
}

#[tokio::test]
async fn prepare_schema_recreates_templates_after_repair_migration_was_already_applied() {
    let db = test_db().await;

    sqlx::query("DROP TABLE templates")
        .execute(db.pool())
        .await
        .unwrap();

    prepare_schema(&db).await.unwrap();

    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM templates")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(row_count > 0);

    let icon_json: String =
        sqlx::query_scalar("SELECT icon_json FROM templates ORDER BY id LIMIT 1")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        icon_json,
        r##"{"type":"icon","value":"notebook-tabs","color":"#9ca3af"}"##
    );
}

#[tokio::test]
async fn prepare_schema_seeds_templates_when_repair_migration_creates_missing_table() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(
        &db,
        anlg_db_migrate::DbSchema {
            steps: &APP_MIGRATION_STEPS[..3],
            validate_cloudsync_table: cloudsync_alter_guard_required,
        },
    )
    .await
    .unwrap();

    sqlx::query("DROP TABLE templates")
        .execute(db.pool())
        .await
        .unwrap();

    prepare_schema(&db).await.unwrap();

    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM templates")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(row_count > 0);
}

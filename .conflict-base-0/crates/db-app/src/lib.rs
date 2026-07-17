#![forbid(unsafe_code)]

mod calendar_ops;
mod calendar_types;
mod cloudsync;
mod e2ee;
mod event_ops;
mod event_types;
mod legacy_import;
mod session_ops;
mod session_types;
mod template_ops;
mod template_types;

pub use calendar_ops::*;
pub use calendar_types::*;
pub use cloudsync::*;
pub use e2ee::*;
pub use event_ops::*;
pub use event_types::*;
pub use legacy_import::*;
pub use session_ops::*;
pub use session_types::*;
pub use template_ops::*;
pub use template_types::*;

pub const APP_MIGRATION_STEPS: &[hypr_db_migrate::MigrationStep] = &[
    hypr_db_migrate::MigrationStep {
        id: "20260413020000_templates",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260413020000_templates.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260414120000_calendars_events",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260414120000_calendars_events.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260524000000_default_templates",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260524000000_default_templates.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260624000000_repair_templates",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260624000000_repair_templates.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260710223922_canonical_data_model",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260710223922_canonical_data_model.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260710231809_import_target_audit",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260710231809_import_target_audit.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260711000000_calendar_event_tombstones",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260711000000_calendar_event_tombstones.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260712170000_template_icons",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260712170000_template_icons.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260713164500_repair_empty_session_titles",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260713164500_repair_empty_session_titles.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260714120000_search_index_queue",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260714120000_search_index_queue.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260714120100_search_index_sessions_triggers",
        scope: hypr_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "sessions",
        },
        sql: include_str!("../migrations/20260714120100_search_index_sessions_triggers.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260714120200_search_index_session_documents_triggers",
        scope: hypr_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "session_documents",
        },
        sql: include_str!(
            "../migrations/20260714120200_search_index_session_documents_triggers.sql"
        ),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260714120300_search_index_transcripts_triggers",
        scope: hypr_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "transcripts",
        },
        sql: include_str!("../migrations/20260714120300_search_index_transcripts_triggers.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260714120400_search_index_humans_triggers",
        scope: hypr_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "humans",
        },
        sql: include_str!("../migrations/20260714120400_search_index_humans_triggers.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260714120500_search_index_organizations_triggers",
        scope: hypr_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "organizations",
        },
        sql: include_str!("../migrations/20260714120500_search_index_organizations_triggers.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260716120000_personal_workspaces",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260716120000_personal_workspaces.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260716130000_cloudsync_session_evictions",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260716130000_cloudsync_session_evictions.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260716173000_shared_session_cache",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260716173000_shared_session_cache.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260717120000_e2ee_replica",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260717120000_e2ee_replica.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260717140000_attachment_local_state",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260717140000_attachment_local_state.sql"),
    },
];

pub fn schema() -> hypr_db_migrate::DbSchema {
    hypr_db_migrate::DbSchema {
        steps: APP_MIGRATION_STEPS,
        validate_cloudsync_table: cloudsync_alter_guard_required,
    }
}

#[derive(Debug)]
pub enum AppSchemaError {
    Migrate(hypr_db_migrate::MigrateError),
    Sqlx(sqlx::Error),
    CloudsyncWorkspace(CloudsyncWorkspaceError),
}

impl std::fmt::Display for AppSchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Migrate(error) => write!(f, "{error}"),
            Self::Sqlx(error) => write!(f, "{error}"),
            Self::CloudsyncWorkspace(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AppSchemaError {}

impl From<hypr_db_migrate::MigrateError> for AppSchemaError {
    fn from(error: hypr_db_migrate::MigrateError) -> Self {
        Self::Migrate(error)
    }
}

impl From<sqlx::Error> for AppSchemaError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sqlx(error)
    }
}

impl From<CloudsyncWorkspaceError> for AppSchemaError {
    fn from(error: CloudsyncWorkspaceError) -> Self {
        Self::CloudsyncWorkspace(error)
    }
}

pub async fn prepare_schema(db: &hypr_db_core::Db) -> Result<(), AppSchemaError> {
    let templates_missing_before_migration = !templates_table_exists(db.pool()).await?;
    hypr_db_migrate::migrate(db, schema()).await?;
    repair_missing_core_tables(db.pool(), templates_missing_before_migration).await?;
    ensure_cloudsync_workspace_binding(db.pool()).await?;
    Ok(())
}

async fn templates_table_exists(pool: &sqlx::SqlitePool) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM sqlite_master
            WHERE type = 'table' AND name = 'templates'
        )",
    )
    .fetch_one(pool)
    .await
}

async fn repair_missing_core_tables(
    pool: &sqlx::SqlitePool,
    templates_missing_before_migration: bool,
) -> Result<(), sqlx::Error> {
    if !templates_table_exists(pool).await? {
        sqlx::query(include_str!("../migrations/20260413020000_templates.sql"))
            .execute(pool)
            .await?;
    }

    let has_icon_json = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('templates') WHERE name = 'icon_json')",
    )
    .fetch_one(pool)
    .await?;
    if !has_icon_json {
        sqlx::query(include_str!(
            "../migrations/20260712170000_template_icons.sql"
        ))
        .execute(pool)
        .await?;
    }

    if templates_missing_before_migration {
        sqlx::query(include_str!(
            "../migrations/20260524000000_default_templates.sql"
        ))
        .execute(pool)
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core::Db;
    use sqlx::Row;

    async fn test_db() -> Db {
        let db = Db::open(hypr_db_core::DbOpenOptions {
            storage: hypr_db_core::DbStorage::Memory,
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        prepare_schema(&db).await.unwrap();
        db
    }

    async fn initialize_enabled_cloudsync_tables(db: &Db) {
        for table in cloudsync_table_registry()
            .iter()
            .filter(|table| table.enabled)
        {
            db.cloudsync_init(
                &table.table_name,
                table.crdt_algo.as_deref(),
                table.init_flags,
            )
            .await
            .unwrap();
        }
    }

    fn migration_steps_before(id: &str) -> &'static [hypr_db_migrate::MigrationStep] {
        let index = APP_MIGRATION_STEPS
            .iter()
            .position(|step| step.id == id)
            .unwrap();
        &APP_MIGRATION_STEPS[..index]
    }

    async fn test_db_without_default_templates() -> Db {
        let db = Db::open(hypr_db_core::DbOpenOptions {
            storage: hypr_db_core::DbStorage::Memory,
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        hypr_db_migrate::migrate(
            &db,
            hypr_db_migrate::DbSchema {
                steps: &APP_MIGRATION_STEPS[..2],
                validate_cloudsync_table: cloudsync_alter_guard_required,
            },
        )
        .await
        .unwrap();
        sqlx::query(include_str!(
            "../migrations/20260712170000_template_icons.sql"
        ))
        .execute(db.pool())
        .await
        .unwrap();
        db
    }

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
                "app_settings",
                "attachment_local_state",
                "calendars",
                "chat_groups",
                "chat_messages",
                "cloudsync_session_evictions",
                "cloudsync_writable_workspaces",
                "daily_notes",
                "e2ee_local_state",
                "e2ee_records",
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
                "session_tags",
                "sessions",
                "shared_session_cache",
                "storage_migration_state",
                "tags",
                "templates",
                "transcripts",
                "workspace_memberships",
                "workspaces",
            ]
        );
    }

    #[tokio::test]
    async fn personal_workspace_migration_preserves_existing_session_workspace_ids() {
        let db = Db::connect_memory_plain().await.unwrap();
        hypr_db_migrate::migrate(
            &db,
            hypr_db_migrate::DbSchema {
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

        hypr_db_migrate::migrate(&db, schema()).await.unwrap();
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
                hypr_db_core::cloudsync_is_enabled_on(db.pool(), table_name)
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
        hypr_db_migrate::migrate(
            &db,
            hypr_db_migrate::DbSchema {
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
                hypr_db_core::cloudsync_is_enabled_on(db.pool(), &table.table_name)
                    .await
                    .unwrap()
            );
        }
    }

    #[tokio::test]
    async fn migration_repairs_empty_titles_from_summary_headings() {
        let db = Db::connect_memory_plain().await.unwrap();
        hypr_db_migrate::migrate(
            &db,
            hypr_db_migrate::DbSchema {
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

        hypr_db_migrate::migrate(&db, schema()).await.unwrap();

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
        hypr_db_migrate::migrate(
            &db,
            hypr_db_migrate::DbSchema {
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

        hypr_db_migrate::migrate(&db, schema()).await.unwrap();

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
        hypr_db_migrate::migrate(
            &db,
            hypr_db_migrate::DbSchema {
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

    #[test]
    fn cloudsync_registry_enables_only_the_encrypted_replica() {
        let registry = cloudsync_table_registry();
        let enabled: Vec<&str> = registry
            .iter()
            .filter(|table| table.enabled)
            .map(|table| table.table_name.as_str())
            .collect();

        assert_eq!(registry.len(), 20);
        assert_eq!(enabled, vec!["e2ee_records"]);
        assert!(
            !registry
                .iter()
                .any(|table| table.table_name == "migration_import_runs")
        );
        assert!(!registry.iter().any(|table| {
            matches!(
                table.table_name.as_str(),
                "search_index_dirty" | "search_index_state"
            )
        }));
        assert!(
            registry
                .iter()
                .any(|table| { table.table_name == "workspaces" && !table.enabled })
        );
        assert!(
            registry
                .iter()
                .any(|table| { table.table_name == "workspace_memberships" && !table.enabled })
        );
        assert!(cloudsync_alter_guard_required("sessions"));
        assert!(cloudsync_alter_guard_required("e2ee_records"));
        assert!(!cloudsync_alter_guard_required("workspaces"));
        assert!(!cloudsync_alter_guard_required("workspace_memberships"));
        assert!(!cloudsync_alter_guard_required("calendars"));
    }

    #[test]
    fn shared_session_cache_is_plain_and_excluded_from_cloudsync() {
        let migration = APP_MIGRATION_STEPS
            .iter()
            .find(|step| step.id == "20260716173000_shared_session_cache")
            .unwrap();

        assert_eq!(migration.scope, hypr_db_migrate::MigrationScope::Plain);
        assert!(
            !cloudsync_table_registry()
                .iter()
                .any(|table| table.table_name == "shared_session_cache")
        );
        assert!(!cloudsync_alter_guard_required("shared_session_cache"));
    }

    #[test]
    fn attachment_local_state_is_plain_and_excluded_from_cloudsync() {
        let migration = APP_MIGRATION_STEPS
            .iter()
            .find(|step| step.id == "20260717140000_attachment_local_state")
            .unwrap();

        assert_eq!(migration.scope, hypr_db_migrate::MigrationScope::Plain);
        assert!(
            !cloudsync_table_registry()
                .iter()
                .any(|table| table.table_name == "attachment_local_state")
        );
        assert!(!E2EE_DOMAIN_TABLES.contains(&"attachment_local_state"));
        assert!(!cloudsync_alter_guard_required("attachment_local_state"));
    }

    #[tokio::test]
    async fn shared_session_cache_enforces_snapshot_contract() {
        let db = test_db().await;
        let insert = "INSERT INTO shared_session_cache (
            share_id,
            viewer_user_id,
            workspace_id,
            session_id,
            schema_version,
            content_revision,
            title,
            body_json,
            capability,
            manage_access,
            access_version,
            published_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

        sqlx::query(insert)
            .bind("share-1")
            .bind("viewer-1")
            .bind("workspace-1")
            .bind("session-1")
            .bind(1_i64)
            .bind(3_i64)
            .bind("Shared note")
            .bind(r#"{"type":"doc","content":[{"type":"paragraph"}]}"#)
            .bind("commenter")
            .bind(1_i64)
            .bind(4_i64)
            .bind("2026-07-16T17:30:00.000Z")
            .execute(db.pool())
            .await
            .unwrap();

        let cached: (i64, String, i64, i64) = sqlx::query_as(
            "SELECT content_revision, capability, manage_access, access_version
             FROM shared_session_cache
             WHERE viewer_user_id = 'viewer-1' AND share_id = 'share-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(cached, (3, "commenter".to_string(), 1, 4));

        sqlx::query(insert)
            .bind("share-1")
            .bind("viewer-2")
            .bind("workspace-1")
            .bind("session-1")
            .bind(1_i64)
            .bind(3_i64)
            .bind("Shared note")
            .bind(r#"{"type":"doc"}"#)
            .bind("viewer")
            .bind(0_i64)
            .bind(4_i64)
            .bind("2026-07-16T17:30:00.000Z")
            .execute(db.pool())
            .await
            .unwrap();
        let viewer_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM shared_session_cache WHERE share_id = 'share-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(viewer_count, 2);

        for (
            share_id,
            viewer_user_id,
            workspace_id,
            session_id,
            schema_version,
            content_revision,
            title,
            body_json,
            capability,
            manage_access,
            access_version,
            published_at,
        ) in [
            (
                "share-schema",
                "viewer-1",
                "workspace-1",
                "session-1",
                2,
                1,
                "Shared note",
                r#"{"type":"doc"}"#,
                "viewer",
                0,
                1,
                "2026-07-16T17:30:00.000Z",
            ),
            (
                "share-revision",
                "viewer-1",
                "workspace-1",
                "session-1",
                1,
                0,
                "Shared note",
                r#"{"type":"doc"}"#,
                "viewer",
                0,
                1,
                "2026-07-16T17:30:00.000Z",
            ),
            (
                "share-body",
                "viewer-1",
                "workspace-1",
                "session-1",
                1,
                1,
                "Shared note",
                r#"{"type":"paragraph"}"#,
                "viewer",
                0,
                1,
                "2026-07-16T17:30:00.000Z",
            ),
            (
                "share-capability",
                "viewer-1",
                "workspace-1",
                "session-1",
                1,
                1,
                "Shared note",
                r#"{"type":"doc"}"#,
                "owner",
                0,
                1,
                "2026-07-16T17:30:00.000Z",
            ),
            (
                "share-manage",
                "viewer-1",
                "workspace-1",
                "session-1",
                1,
                1,
                "Shared note",
                r#"{"type":"doc"}"#,
                "viewer",
                2,
                1,
                "2026-07-16T17:30:00.000Z",
            ),
            (
                "share-access-version",
                "viewer-1",
                "workspace-1",
                "session-1",
                1,
                1,
                "Shared note",
                r#"{"type":"doc"}"#,
                "viewer",
                0,
                0,
                "2026-07-16T17:30:00.000Z",
            ),
            (
                "share-id",
                "viewer-1",
                " ",
                "session-1",
                1,
                1,
                "Shared note",
                r#"{"type":"doc"}"#,
                "viewer",
                0,
                1,
                "2026-07-16T17:30:00.000Z",
            ),
            (
                "share-viewer",
                " ",
                "workspace-1",
                "session-1",
                1,
                1,
                "Shared note",
                r#"{"type":"doc"}"#,
                "viewer",
                0,
                1,
                "2026-07-16T17:30:00.000Z",
            ),
        ] {
            let result = sqlx::query(insert)
                .bind(share_id)
                .bind(viewer_user_id)
                .bind(workspace_id)
                .bind(session_id)
                .bind(schema_version)
                .bind(content_revision)
                .bind(title)
                .bind(body_json)
                .bind(capability)
                .bind(manage_access)
                .bind(access_version)
                .bind(published_at)
                .execute(db.pool())
                .await;
            assert!(result.is_err(), "invalid cache row {share_id} was accepted");
        }

        let malformed_json = sqlx::query(insert)
            .bind("share-json")
            .bind("viewer-1")
            .bind("workspace-1")
            .bind("session-1")
            .bind(1_i64)
            .bind(1_i64)
            .bind("Shared note")
            .bind("not-json")
            .bind("viewer")
            .bind(0_i64)
            .bind(1_i64)
            .bind("2026-07-16T17:30:00.000Z")
            .execute(db.pool())
            .await;
        assert!(malformed_json.is_err());
    }

    #[test]
    fn search_index_trigger_migrations_are_cloudsync_guarded() {
        let queue_step = APP_MIGRATION_STEPS
            .iter()
            .find(|step| step.id == "20260714120000_search_index_queue")
            .unwrap();
        assert_eq!(queue_step.scope, hypr_db_migrate::MigrationScope::Plain);

        for (id, table_name) in [
            ("20260714120100_search_index_sessions_triggers", "sessions"),
            (
                "20260714120200_search_index_session_documents_triggers",
                "session_documents",
            ),
            (
                "20260714120300_search_index_transcripts_triggers",
                "transcripts",
            ),
            ("20260714120400_search_index_humans_triggers", "humans"),
            (
                "20260714120500_search_index_organizations_triggers",
                "organizations",
            ),
        ] {
            let step = APP_MIGRATION_STEPS
                .iter()
                .find(|step| step.id == id)
                .unwrap();
            assert_eq!(
                step.scope,
                hypr_db_migrate::MigrationScope::CloudsyncAlter { table_name }
            );
        }
    }

    #[tokio::test]
    async fn search_index_queue_coalesces_changes_and_tracks_session_moves() {
        let db = test_db().await;

        sqlx::query(
            "INSERT INTO sessions (id, title) VALUES ('session-1', 'One'), ('session-2', 'Two')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query("DELETE FROM search_index_dirty")
            .execute(db.pool())
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO session_documents (id, session_id, body) VALUES ('document-1', 'session-1', 'one')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query("UPDATE session_documents SET body = 'two' WHERE id = 'document-1'")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "UPDATE session_documents SET session_id = 'session-2' WHERE id = 'document-1'",
        )
        .execute(db.pool())
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO transcripts (id, session_id, words_json) VALUES ('transcript-1', 'session-1', '[]')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query("UPDATE transcripts SET session_id = 'session-2' WHERE id = 'transcript-1'")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("DELETE FROM transcripts WHERE id = 'transcript-1'")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("DELETE FROM session_documents WHERE id = 'document-1'")
            .execute(db.pool())
            .await
            .unwrap();

        let rows = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT entity_type, entity_id, generation
             FROM search_index_dirty
             ORDER BY entity_type, entity_id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();

        assert_eq!(
            rows,
            vec![
                ("session".to_string(), "session-1".to_string(), 5),
                ("session".to_string(), "session-2".to_string(), 4),
            ]
        );
    }

    #[tokio::test]
    async fn search_index_queue_tracks_entity_lifecycle_and_starts_unversioned() {
        let db = test_db().await;

        let projection_version: i64 = sqlx::query_scalar(
            "SELECT projection_version FROM search_index_state WHERE id = 'default'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(projection_version, 0);

        sqlx::query("INSERT INTO sessions (id, title) VALUES ('session-1', 'One')")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO humans (id, name) VALUES ('human-1', 'Ada')")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO organizations (id, name) VALUES ('organization-1', 'Acme')")
            .execute(db.pool())
            .await
            .unwrap();

        sqlx::query(
            "UPDATE sessions SET deleted_at = '2026-07-14T00:00:00Z' WHERE id = 'session-1'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query("UPDATE humans SET memo = 'Updated' WHERE id = 'human-1'")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("DELETE FROM organizations WHERE id = 'organization-1'")
            .execute(db.pool())
            .await
            .unwrap();

        let rows = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT entity_type, entity_id, generation
             FROM search_index_dirty
             ORDER BY entity_type, entity_id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();

        assert_eq!(
            rows,
            vec![
                ("human".to_string(), "human-1".to_string(), 2),
                ("organization".to_string(), "organization-1".to_string(), 2,),
                ("session".to_string(), "session-1".to_string(), 2),
            ]
        );
    }

    #[tokio::test]
    async fn registered_tables_match_cloudsync_schema_requirements() {
        let db = test_db().await;

        for table in cloudsync_table_registry() {
            let rows = sqlx::query(
                "SELECT name, type, \"notnull\", dflt_value, pk
                 FROM pragma_table_info(?)
                 ORDER BY cid",
            )
            .bind(&table.table_name)
            .fetch_all(db.pool())
            .await
            .unwrap();

            let pk_columns: Vec<_> = rows
                .iter()
                .filter(|row| row.get::<i64, _>("pk") > 0)
                .collect();
            assert_eq!(
                pk_columns.len(),
                1,
                "{} must have one primary key",
                table.table_name
            );

            let pk = pk_columns[0];
            assert_eq!(pk.get::<String, _>("name"), "id", "{}", table.table_name);
            assert_eq!(
                pk.get::<String, _>("type").to_uppercase(),
                "TEXT",
                "{}",
                table.table_name
            );
            assert_eq!(pk.get::<i64, _>("notnull"), 1, "{}", table.table_name);

            for row in rows
                .iter()
                .filter(|row| row.get::<i64, _>("pk") == 0 && row.get::<i64, _>("notnull") == 1)
            {
                assert!(
                    row.get::<Option<String>, _>("dflt_value").is_some(),
                    "{}.{} must define a DEFAULT value for SQLite Sync compatibility",
                    table.table_name,
                    row.get::<String, _>("name")
                );
            }
        }
    }

    #[tokio::test]
    async fn calendar_roundtrip() {
        let db = test_db().await;

        upsert_calendar(
            db.pool(),
            UpsertCalendar {
                id: "cal1",
                tracking_id_calendar: "tracking-cal-1",
                name: "Work",
                enabled: true,
                provider: "google",
                source: "team",
                color: "#123456",
                connection_id: "conn-1",
            },
        )
        .await
        .unwrap();

        let row = get_calendar(db.pool(), "cal1").await.unwrap().unwrap();
        assert_eq!(row.name, "Work");
        assert!(row.enabled);

        let rows = list_calendars(db.pool()).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "cal1");
    }

    #[tokio::test]
    async fn event_roundtrip() {
        let db = test_db().await;

        upsert_event(
            db.pool(),
            UpsertEvent {
                id: "evt1",
                tracking_id_event: "tracking-evt-1",
                calendar_id: "cal1",
                title: "Standup",
                started_at: "2026-04-15T09:00:00Z",
                ended_at: "2026-04-15T09:30:00Z",
                location: "",
                meeting_link: "https://meet.example/1",
                description: "Daily sync",
                note: "",
                recurrence_series_id: "series-1",
                has_recurrence_rules: true,
                is_all_day: false,
                provider: "google",
                participants_json: Some("[{\"email\":\"a@example.com\"}]"),
            },
        )
        .await
        .unwrap();

        let row = get_event(db.pool(), "evt1").await.unwrap().unwrap();
        assert_eq!(row.title, "Standup");
        assert_eq!(row.calendar_id, "cal1");

        let rows = list_events(db.pool()).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "evt1");
    }

    #[tokio::test]
    async fn template_roundtrip() {
        let db = test_db().await;

        upsert_template(
            db.pool(),
            UpsertTemplate {
                id: "template-1",
                title: "Standup",
                description: "Daily sync",
                pinned: true,
                pin_order: Some(2),
                category: Some("meetings"),
                targets_json: Some("[\"engineering\"]"),
                sections_json: "[{\"title\":\"Notes\",\"description\":\"...\"}]",
            },
        )
        .await
        .unwrap();

        let row = get_template(db.pool(), "template-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.title, "Standup");
        assert_eq!(row.targets_json.as_deref(), Some("[\"engineering\"]"));
        assert_eq!(
            row.sections_json,
            "[{\"title\":\"Notes\",\"description\":\"...\"}]"
        );
    }

    #[tokio::test]
    async fn migrations_seed_default_templates_without_overwriting_existing_rows() {
        let db = Db::connect_memory_plain().await.unwrap();
        hypr_db_migrate::migrate(
            &db,
            hypr_db_migrate::DbSchema {
                steps: &APP_MIGRATION_STEPS[..1],
                validate_cloudsync_table: cloudsync_alter_guard_required,
            },
        )
        .await
        .unwrap();

        upsert_template(
            db.pool(),
            UpsertTemplate {
                id: "default-daily-standup",
                title: "Custom Standup",
                description: "Keep user edit",
                pinned: true,
                pin_order: Some(1),
                category: Some("Custom"),
                targets_json: Some("[\"Team\"]"),
                sections_json: "[{\"title\":\"Custom\",\"description\":\"Keep\"}]",
            },
        )
        .await
        .unwrap();

        hypr_db_migrate::migrate(&db, schema()).await.unwrap();

        let rows = list_templates(db.pool()).await.unwrap();
        assert_eq!(rows.len(), 17);
        assert_eq!(
            rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            vec![
                "default-board-meeting",
                "default-brainstorming-session",
                "default-client-kickoff",
                "default-customer-discovery",
                "default-daily-standup",
                "default-executive-briefing",
                "default-incident-postmortem",
                "default-investor-pitch",
                "default-lecture-notes",
                "default-one-on-one-meeting",
                "default-performance-review",
                "default-product-roadmap-review",
                "default-project-kickoff",
                "default-sales-discovery-call",
                "default-sprint-planning",
                "default-sprint-retrospective",
                "default-technical-design-review",
            ]
        );

        let custom_row = get_template(db.pool(), "default-daily-standup")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(custom_row.title, "Custom Standup");
        assert_eq!(custom_row.description, "Keep user edit");

        let seeded_row = get_template(db.pool(), "default-sales-discovery-call")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(seeded_row.title, "Sales Discovery Call");
        assert_eq!(
            seeded_row.targets_json.as_deref(),
            Some("[\"Account Executive\",\"Sales Rep\",\"BDR\"]")
        );
    }

    #[tokio::test]
    async fn list_templates_returns_all_ordered_by_id() {
        let db = test_db_without_default_templates().await;

        upsert_template(
            db.pool(),
            UpsertTemplate {
                id: "template-2",
                title: "Two",
                description: "",
                pinned: false,
                pin_order: None,
                category: None,
                targets_json: None,
                sections_json: "[]",
            },
        )
        .await
        .unwrap();

        upsert_template(
            db.pool(),
            UpsertTemplate {
                id: "template-1",
                title: "One",
                description: "",
                pinned: false,
                pin_order: None,
                category: None,
                targets_json: None,
                sections_json: "[]",
            },
        )
        .await
        .unwrap();

        let rows = list_templates(db.pool()).await.unwrap();
        let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();

        assert_eq!(ids, vec!["template-1", "template-2"]);
    }

    #[tokio::test]
    async fn template_upsert_replaces_existing_row_by_id() {
        let db = test_db_without_default_templates().await;

        upsert_template(
            db.pool(),
            UpsertTemplate {
                id: "template-1",
                title: "First",
                description: "A",
                pinned: false,
                pin_order: None,
                category: None,
                targets_json: None,
                sections_json: "[]",
            },
        )
        .await
        .unwrap();

        upsert_template(
            db.pool(),
            UpsertTemplate {
                id: "template-1",
                title: "Second",
                description: "B",
                pinned: true,
                pin_order: Some(5),
                category: Some("sales"),
                targets_json: Some("[\"exec\"]"),
                sections_json: "[{\"title\":\"Summary\",\"description\":\"Updated\"}]",
            },
        )
        .await
        .unwrap();

        let row = get_template(db.pool(), "template-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.title, "Second");
        assert_eq!(row.description, "B");
        assert!(row.pinned);
        assert_eq!(row.pin_order, Some(5));
        assert_eq!(row.category.as_deref(), Some("sales"));
        assert_eq!(row.targets_json.as_deref(), Some("[\"exec\"]"));
        assert_eq!(
            row.sections_json,
            "[{\"title\":\"Summary\",\"description\":\"Updated\"}]"
        );
        assert_eq!(list_templates(db.pool()).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn template_delete_removes_row() {
        let db = test_db().await;

        upsert_template(
            db.pool(),
            UpsertTemplate {
                id: "template-1",
                title: "Delete Me",
                description: "",
                pinned: false,
                pin_order: None,
                category: None,
                targets_json: None,
                sections_json: "[]",
            },
        )
        .await
        .unwrap();

        delete_template(db.pool(), "template-1").await.unwrap();

        assert!(
            get_template(db.pool(), "template-1")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn template_insert_if_missing_preserves_existing_row() {
        let db = test_db().await;

        upsert_template(
            db.pool(),
            UpsertTemplate {
                id: "template-1",
                title: "Original",
                description: "A",
                pinned: false,
                pin_order: None,
                category: None,
                targets_json: None,
                sections_json: "[]",
            },
        )
        .await
        .unwrap();

        let inserted = insert_template_if_missing(
            db.pool(),
            UpsertTemplate {
                id: "template-1",
                title: "Replacement",
                description: "B",
                pinned: true,
                pin_order: Some(4),
                category: Some("meetings"),
                targets_json: Some("[\"exec\"]"),
                sections_json: "[{\"title\":\"Summary\",\"description\":\"Updated\"}]",
            },
        )
        .await
        .unwrap();

        assert!(!inserted);

        let row = get_template(db.pool(), "template-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.title, "Original");
        assert_eq!(row.description, "A");
        assert!(!row.pinned);
        assert_eq!(row.pin_order, None);
        assert_eq!(row.category, None);
        assert_eq!(row.targets_json, None);
        assert_eq!(row.sections_json, "[]");
    }
}

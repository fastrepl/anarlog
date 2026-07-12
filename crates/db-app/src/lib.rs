#![forbid(unsafe_code)]

mod calendar_ops;
mod calendar_types;
mod cloudsync;
mod event_ops;
mod event_types;
mod legacy_import;
mod template_ops;
mod template_types;

pub use calendar_ops::*;
pub use calendar_types::*;
pub use cloudsync::*;
pub use event_ops::*;
pub use event_types::*;
pub use legacy_import::*;
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
}

impl std::fmt::Display for AppSchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Migrate(error) => write!(f, "{error}"),
            Self::Sqlx(error) => write!(f, "{error}"),
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

pub async fn prepare_schema(db: &hypr_db_core::Db) -> Result<(), AppSchemaError> {
    let templates_missing_before_migration = !templates_table_exists(db.pool()).await?;
    hypr_db_migrate::migrate(db, schema()).await?;
    repair_missing_core_tables(db.pool(), templates_missing_before_migration).await?;
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
                "calendars",
                "chat_groups",
                "chat_messages",
                "daily_notes",
                "entity_mentions",
                "events",
                "humans",
                "migration_import_items",
                "migration_import_runs",
                "migration_import_targets",
                "organizations",
                "session_attachments",
                "session_documents",
                "session_participants",
                "session_tags",
                "sessions",
                "storage_migration_state",
                "tags",
                "templates",
                "transcripts",
            ]
        );
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
    fn cloudsync_registry_declares_domain_tables_disabled_until_sync_rollout() {
        let registry = cloudsync_table_registry();

        assert_eq!(registry.len(), 17);
        assert!(registry.iter().all(|table| !table.enabled));
        assert!(registry.iter().any(|table| table.table_name == "sessions"));
        assert!(
            registry
                .iter()
                .any(|table| table.table_name == "session_documents")
        );
        assert!(
            !registry
                .iter()
                .any(|table| table.table_name == "migration_import_runs")
        );
        assert!(!cloudsync_alter_guard_required("sessions"));
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

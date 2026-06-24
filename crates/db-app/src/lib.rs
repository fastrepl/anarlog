#![forbid(unsafe_code)]

mod calendar_ops;
mod calendar_types;
mod cloudsync;
mod event_ops;
mod event_types;
mod template_ops;
mod template_types;

pub use calendar_ops::*;
pub use calendar_types::*;
pub use cloudsync::*;
pub use event_ops::*;
pub use event_types::*;
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
];

pub fn schema() -> hypr_db_migrate::DbSchema {
    hypr_db_migrate::DbSchema {
        steps: APP_MIGRATION_STEPS,
        validate_cloudsync_table: cloudsync_alter_guard_required,
    }
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
        hypr_db_migrate::migrate(&db, schema()).await.unwrap();
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
            vec!["_sqlx_migrations", "calendars", "events", "templates"]
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

    #[test]
    fn cloudsync_registry_starts_with_templates_disabled() {
        let registry = cloudsync_table_registry();

        assert_eq!(registry.len(), 1);
        assert_eq!(registry[0].table_name, "templates");
        assert!(!registry[0].enabled);
        assert!(!cloudsync_alter_guard_required("templates"));
    }

    #[tokio::test]
    async fn templates_table_matches_cloudsync_schema_requirements() {
        let db = test_db().await;

        let rows = sqlx::query(
            "SELECT name, type, \"notnull\", dflt_value, pk
             FROM pragma_table_info('templates')
             ORDER BY cid",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();

        let pk_columns: Vec<_> = rows
            .iter()
            .filter(|row| row.get::<i64, _>("pk") > 0)
            .collect();
        assert_eq!(pk_columns.len(), 1);

        let pk = pk_columns[0];
        assert_eq!(pk.get::<String, _>("name"), "id");
        assert_eq!(pk.get::<String, _>("type").to_uppercase(), "TEXT");
        assert_eq!(pk.get::<i64, _>("notnull"), 1);

        for row in rows
            .iter()
            .filter(|row| row.get::<i64, _>("pk") == 0 && row.get::<i64, _>("notnull") == 1)
        {
            assert!(
                row.get::<Option<String>, _>("dflt_value").is_some(),
                "column {} must define a DEFAULT value for SQLite Sync compatibility",
                row.get::<String, _>("name")
            );
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

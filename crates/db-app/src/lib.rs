#![forbid(unsafe_code)]

mod activity_ops;
mod activity_types;
mod cloudsync;
mod daily_note_ops;
mod daily_note_types;
mod daily_summary_ops;
mod daily_summary_types;
mod template_ops;
mod template_types;

pub use activity_ops::*;
pub use activity_types::*;
pub use cloudsync::*;
pub use daily_note_ops::*;
pub use daily_note_types::*;
pub use daily_summary_ops::*;
pub use daily_summary_types::*;
pub use template_ops::*;
pub use template_types::*;

use sqlx::SqlitePool;

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core2::Db3;
    use sqlx::Row;

    async fn test_db() -> Db3 {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();
        db
    }

    #[tokio::test]
    async fn migrations_apply_cleanly() {
        let db = test_db().await;

        let tables: Vec<String> = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '_sqlx%' ORDER BY name",
        )
        .fetch_all(db.pool().as_ref())
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.0)
        .collect();

        assert_eq!(
            tables,
            vec![
                "activity_observation_analyses",
                "activity_observation_events",
                "activity_screenshots",
                "daily_notes",
                "daily_summaries",
                "templates",
            ]
        );
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
        .fetch_all(db.pool().as_ref())
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
    async fn daily_note_roundtrip() {
        let db = test_db().await;

        upsert_daily_note_body(db.pool(), "dn1", "2026-04-11", "{\"type\":\"doc\"}", "u1")
            .await
            .unwrap();

        let row = get_daily_note(db.pool(), "dn1").await.unwrap().unwrap();
        assert_eq!(row.date, "2026-04-11");
        assert_eq!(row.user_id, "u1");

        let by_date = get_daily_note_by_date(db.pool(), "2026-04-11", "u1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_date.id, "dn1");
    }

    #[tokio::test]
    async fn get_or_create_daily_note_is_idempotent() {
        let db = test_db().await;

        let first = get_or_create_daily_note(db.pool(), "dn1", "2026-04-11", "u1")
            .await
            .unwrap();
        let second = get_or_create_daily_note(db.pool(), "ignored", "2026-04-11", "u1")
            .await
            .unwrap();

        assert_eq!(first.id, "dn1");
        assert_eq!(second.id, "dn1");
    }

    #[tokio::test]
    async fn list_daily_notes_in_range_filters_by_user() {
        let db = test_db().await;

        upsert_daily_note_body(db.pool(), "dn1", "2026-04-10", "{}", "u1")
            .await
            .unwrap();
        upsert_daily_note_body(db.pool(), "dn2", "2026-04-11", "{}", "u1")
            .await
            .unwrap();
        upsert_daily_note_body(db.pool(), "dn3", "2026-04-11", "{}", "u2")
            .await
            .unwrap();

        let rows = list_daily_notes_in_range(db.pool(), "2026-04-10", "2026-04-11", "u1")
            .await
            .unwrap();
        let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();

        assert_eq!(ids, vec!["dn1", "dn2"]);
    }

    #[tokio::test]
    async fn daily_summary_roundtrip() {
        let db = test_db().await;

        upsert_daily_summary(
            db.pool(),
            UpsertDailySummary {
                id: "ds1",
                daily_note_id: "dn1",
                date: "2026-04-11",
                content: "# Summary",
                timeline_json: "[{\"time\":\"09:00\"}]",
                topics_json: "[\"db\"]",
                status: "ready",
                source_cursor_ms: 123,
                source_fingerprint: "fp1",
                generation_error: "",
                generated_at: "2026-04-11T09:30:00Z",
            },
        )
        .await
        .unwrap();

        let row = get_daily_summary(db.pool(), "ds1").await.unwrap().unwrap();
        assert_eq!(row.status, "ready");
        assert_eq!(row.source_cursor_ms, 123);

        let by_date = get_daily_summary_by_date(db.pool(), "2026-04-11", "dn1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_date.id, "ds1");
    }

    #[tokio::test]
    async fn daily_summary_upsert_replaces_existing_row_for_daily_note() {
        let db = test_db().await;

        upsert_daily_summary(
            db.pool(),
            UpsertDailySummary {
                id: "ds1",
                daily_note_id: "dn1",
                date: "2026-04-11",
                content: "first",
                timeline_json: "[]",
                topics_json: "[]",
                status: "ready",
                source_cursor_ms: 10,
                source_fingerprint: "a",
                generation_error: "",
                generated_at: "2026-04-11T09:00:00Z",
            },
        )
        .await
        .unwrap();

        upsert_daily_summary(
            db.pool(),
            UpsertDailySummary {
                id: "ds2",
                daily_note_id: "dn1",
                date: "2026-04-11",
                content: "second",
                timeline_json: "[1]",
                topics_json: "[2]",
                status: "ready",
                source_cursor_ms: 20,
                source_fingerprint: "b",
                generation_error: "",
                generated_at: "2026-04-11T10:00:00Z",
            },
        )
        .await
        .unwrap();

        let row = get_daily_summary_by_date(db.pool(), "2026-04-11", "dn1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.id, "ds1");
        assert_eq!(row.content, "second");
        assert_eq!(row.source_cursor_ms, 20);
    }

    #[tokio::test]
    async fn daily_summary_generating_and_error_helpers_work() {
        let db = test_db().await;

        mark_daily_summary_generating(db.pool(), "ds1", "dn1", "2026-04-11")
            .await
            .unwrap();
        let generating = get_daily_summary(db.pool(), "ds1").await.unwrap().unwrap();
        assert_eq!(generating.status, "generating");

        mark_daily_summary_error(db.pool(), "ds1", "dn1", "2026-04-11", "boom")
            .await
            .unwrap();
        let errored = get_daily_summary_by_date(db.pool(), "2026-04-11", "dn1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(errored.status, "error");
        assert_eq!(errored.generation_error, "boom");

        delete_daily_summary(db.pool(), "ds1").await.unwrap();
        assert!(get_daily_summary(db.pool(), "ds1").await.unwrap().is_none());
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
    async fn list_templates_returns_all_ordered_by_id() {
        let db = test_db().await;

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
        let db = test_db().await;

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

    #[tokio::test]
    async fn observation_event_and_analysis_roundtrip() {
        let db = test_db().await;

        insert_observation_event(
            db.pool(),
            InsertObservationEvent {
                id: "obs_evt_1",
                observation_id: "obs_1",
                occurred_at_ms: 1_000,
                event_kind: "started",
                end_reason: None,
                change_class: Some("structural"),
                app_id: "com.apple.Mail",
                bundle_id: "com.apple.Mail",
                app_name: "Mail",
                activity_kind: "foreground_window",
                window_title: "Compose",
                url: "",
                domain: "",
                text_anchor_identity: "compose:body",
                observation_key: "mail|foreground_window|window:10|compose:body",
                snapshot_json: "{}",
            },
        )
        .await
        .unwrap();

        insert_screenshot(
            db.pool(),
            InsertScreenshot {
                id: "ss_1",
                observation_id: "obs_1",
                screenshot_kind: "settled",
                scheduled_at_ms: 900,
                captured_at_ms: 1_100,
                app_name: "Mail",
                window_title: "Compose",
                mime_type: "image/png",
                width: 100,
                height: 50,
                sha256: "hash",
                image_blob: &[1, 2, 3],
                snapshot_json: "{}",
            },
        )
        .await
        .unwrap();

        insert_observation_analysis(
            db.pool(),
            InsertObservationAnalysis {
                id: "oa_1",
                observation_id: "obs_1",
                screenshot_id: "ss_1",
                screenshot_kind: "settled",
                captured_at_ms: 1_100,
                model_name: "local-vlm",
                prompt_version: "v1",
                app_name: "Mail",
                window_title: "Compose",
                summary: "User is drafting an email reply.",
            },
        )
        .await
        .unwrap();

        let events = list_observation_events_in_range(db.pool(), 0, 2_000)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].observation_id, "obs_1");

        let analyses = list_preferred_observation_analyses_in_range(db.pool(), 0, 2_000)
            .await
            .unwrap();
        assert_eq!(analyses.len(), 1);
        assert_eq!(analyses[0].summary, "User is drafting an email reply.");
        assert_eq!(analyses[0].screenshot_kind, "settled");

        assert_eq!(count_screenshots_since(db.pool(), 0).await.unwrap(), 1);
        assert_eq!(total_screenshot_storage_bytes(db.pool()).await.unwrap(), 3);
    }

    #[tokio::test]
    async fn preferred_analysis_prefers_settled_over_entry() {
        let db = test_db().await;

        insert_observation_analysis(
            db.pool(),
            InsertObservationAnalysis {
                id: "oa_entry",
                observation_id: "obs_1",
                screenshot_id: "ss_entry",
                screenshot_kind: "entry",
                captured_at_ms: 1_000,
                model_name: "local-vlm",
                prompt_version: "v1",
                app_name: "Safari",
                window_title: "Example",
                summary: "Entry view",
            },
        )
        .await
        .unwrap();

        insert_observation_analysis(
            db.pool(),
            InsertObservationAnalysis {
                id: "oa_settled",
                observation_id: "obs_1",
                screenshot_id: "ss_settled",
                screenshot_kind: "settled",
                captured_at_ms: 1_100,
                model_name: "local-vlm",
                prompt_version: "v1",
                app_name: "Safari",
                window_title: "Example",
                summary: "Settled view",
            },
        )
        .await
        .unwrap();

        let analyses = list_preferred_observation_analyses_in_range(db.pool(), 0, 2_000)
            .await
            .unwrap();
        assert_eq!(analyses.len(), 1);
        assert_eq!(analyses[0].summary, "Settled view");
        assert_eq!(analyses[0].screenshot_kind, "settled");
    }
}

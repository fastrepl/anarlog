#![forbid(unsafe_code)]

mod calendar_ops;
mod calendar_types;
mod daily_note_ops;
mod daily_note_types;
mod event_ops;
mod event_types;
mod session_ops;
mod session_types;
mod task_ops;
mod task_types;

pub use calendar_ops::*;
pub use calendar_types::*;
pub use daily_note_ops::*;
pub use daily_note_types::*;
pub use event_ops::*;
pub use event_types::*;
pub use session_ops::*;
pub use session_types::*;
pub use task_ops::*;
pub use task_types::*;

pub const APP2_MIGRATION_STEPS: &[hypr_db_migrate::MigrationStep] = &[
    hypr_db_migrate::MigrationStep {
        id: "20260414120000_calendars_events",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260414120000_calendars_events.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260420120000_sessions",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260420120000_sessions.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260421000000_daily_notes",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260421000000_daily_notes.sql"),
    },
    hypr_db_migrate::MigrationStep {
        id: "20260422000000_tasks",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260422000000_tasks.sql"),
    },
];

pub fn schema() -> hypr_db_migrate::DbSchema {
    hypr_db_migrate::DbSchema {
        steps: APP2_MIGRATION_STEPS,
        validate_cloudsync_table: |_table| false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core::Db;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        hypr_db_migrate::migrate(&db, schema()).await.unwrap();
        db
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
                "calendars",
                "daily_notes",
                "events",
                "sessions",
                "tasks",
            ]
        );
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
        assert_eq!(list_calendars(db.pool()).await.unwrap().len(), 1);
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
        assert_eq!(list_events(db.pool()).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn session_roundtrip() {
        let db = test_db().await;

        upsert_session(
            db.pool(),
            UpsertSession {
                id: "session-1",
                title: "Weekly sync",
                raw_md: "# Notes",
                folder_id: "folder-a",
                event_json: "{\"provider\":\"calendar\"}",
            },
        )
        .await
        .unwrap();

        let row = get_session(db.pool(), "session-1").await.unwrap().unwrap();
        assert_eq!(row.title, "Weekly sync");
        assert_eq!(list_sessions(db.pool()).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn daily_note_roundtrip() {
        let db = test_db().await;

        upsert_daily_note(
            db.pool(),
            UpsertDailyNote {
                date: "2026-04-20",
                content: "before",
            },
        )
        .await
        .unwrap();

        upsert_daily_note(
            db.pool(),
            UpsertDailyNote {
                date: "2026-04-20",
                content: "after",
            },
        )
        .await
        .unwrap();

        let row = get_daily_note(db.pool(), "2026-04-20")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.content, "after");
    }

    #[tokio::test]
    async fn task_roundtrip() {
        let db = test_db().await;

        upsert_task(
            db.pool(),
            UpsertTask {
                id: "task-1",
                source_type: "session",
                source_id: "session-1",
                source_order: 0,
                status: "todo",
                text_preview: "Follow up",
                body_json: "[{\"type\":\"paragraph\"}]",
                due_date: Some("2026-04-25"),
            },
        )
        .await
        .unwrap();

        upsert_task(
            db.pool(),
            UpsertTask {
                id: "task-2",
                source_type: "session",
                source_id: "session-1",
                source_order: 1,
                status: "in_progress",
                text_preview: "Prepare notes",
                body_json: "[{\"type\":\"paragraph\"}]",
                due_date: None,
            },
        )
        .await
        .unwrap();

        move_tasks_to_source(
            db.pool(),
            MoveTaskToSource {
                task_ids: &["task-2"],
                source_type: "daily_note",
                source_id: "2026-04-20",
                starting_source_order: 0,
            },
        )
        .await
        .unwrap();

        let session_tasks = list_tasks_for_source(db.pool(), "session", "session-1")
            .await
            .unwrap();
        assert_eq!(session_tasks.len(), 1);
        assert_eq!(session_tasks[0].id, "task-1");

        let moved = get_task(db.pool(), "task-2").await.unwrap().unwrap();
        assert_eq!(moved.source_type, "daily_note");
        assert_eq!(moved.source_id, "2026-04-20");

        delete_task(db.pool(), "task-1").await.unwrap();
        assert!(get_task(db.pool(), "task-1").await.unwrap().is_none());
    }
}

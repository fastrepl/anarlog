#![forbid(unsafe_code)]

mod activity_ops;
mod activity_types;
mod daily_notes_ops;
mod daily_notes_types;
mod tasks_ops;
mod tasks_types;

pub use activity_ops::*;
pub use activity_types::*;
pub use daily_notes_ops::*;
pub use daily_notes_types::*;
pub use tasks_ops::*;
pub use tasks_types::*;

use sqlx::SqlitePool;

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core2::Db3;

    #[tokio::test]
    async fn migrations_apply_cleanly() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        let tables: Vec<String> = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '_sqlx%' ORDER BY name",
        )
        .fetch_all(db.pool())
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.0)
        .collect();

        assert_eq!(
            tables,
            vec![
                "activity_segments",
                "activity_signals",
                "daily_notes",
                "tasks",
            ]
        );
    }

    #[tokio::test]
    async fn daily_notes_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        upsert_daily_note(db.pool(), "dn1", "2026-04-06", "{\"type\":\"doc\"}", "u1")
            .await
            .unwrap();

        let note = get_daily_note_by_date(db.pool(), "2026-04-06", "u1")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(note.id, "dn1");
        assert_eq!(note.content, "{\"type\":\"doc\"}");
    }

    #[tokio::test]
    async fn tasks_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        upsert_daily_note(db.pool(), "dn1", "2026-04-06", "{}", "u1")
            .await
            .unwrap();
        insert_task(db.pool(), "t1", "dn1", "todo", "Ship db-app2", "0001", "u1")
            .await
            .unwrap();
        update_task_status(db.pool(), "t1", "done").await.unwrap();

        let tasks = list_tasks_by_daily_note(db.pool(), "dn1").await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Ship db-app2");
        assert_eq!(tasks[0].status, "done");
    }

    #[tokio::test]
    async fn activity_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_activity_signal(
            db.pool(),
            InsertActivitySignal {
                id: "sig1",
                occurred_at_ms: 1_000,
                transition_sequence: 1,
                reason: "started",
                app_id: "com.apple.TextEdit",
                bundle_id: "com.apple.TextEdit",
                app_name: "TextEdit",
                activity_kind: "foreground_window",
                window_title: "Notes",
                url: "",
                domain: "",
                content_level: "metadata",
                source: "workspace",
                text_anchor_identity: "",
                fingerprint: "fp1",
                payload_json: "{}",
            },
        )
        .await
        .unwrap();

        upsert_activity_segment(
            db.pool(),
            UpsertActivitySegment {
                id: "seg1",
                started_at_ms: 1_000,
                ended_at_ms: 6_000,
                duration_ms: 5_000,
                date: "2026-04-06",
                semantic_key: "textedit|notes",
                app_id: "com.apple.TextEdit",
                bundle_id: "com.apple.TextEdit",
                app_name: "TextEdit",
                activity_kind: "foreground_window",
                title: "Notes",
                url: "",
                domain: "",
                payload_json: "{}",
                source_signal_start_id: Some("sig1"),
                source_signal_end_id: Some("sig1"),
                linked_task_id: None,
                linked_daily_note_id: None,
            },
        )
        .await
        .unwrap();

        let signals = list_activity_signals_since(db.pool(), 0).await.unwrap();
        let segments = list_activity_segments_for_date(db.pool(), "2026-04-06")
            .await
            .unwrap();

        assert_eq!(signals.len(), 1);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].semantic_key, "textedit|notes");
    }
}

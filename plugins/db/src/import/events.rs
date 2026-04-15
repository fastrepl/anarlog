use std::collections::HashMap;
use std::path::Path;

use hypr_db_app::UpsertEvent;
use sqlx::SqlitePool;

pub async fn import_legacy_events_from_path(pool: &SqlitePool, path: &Path) -> crate::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let events = read_events_file(path)?;
    for event in events {
        hypr_db_app::insert_event_if_missing(
            pool,
            UpsertEvent {
                id: &event.id,
                tracking_id_event: &event.tracking_id_event,
                calendar_id: &event.calendar_id,
                title: &event.title,
                started_at: &event.started_at,
                ended_at: &event.ended_at,
                location: &event.location,
                meeting_link: &event.meeting_link,
                description: &event.description,
                note: &event.note,
                recurrence_series_id: &event.recurrence_series_id,
                has_recurrence_rules: event.has_recurrence_rules,
                is_all_day: event.is_all_day,
                provider: &event.provider,
                participants_json: event.participants_json.as_deref(),
            },
        )
        .await?;
    }

    Ok(())
}

struct LegacyEvent {
    id: String,
    tracking_id_event: String,
    calendar_id: String,
    title: String,
    started_at: String,
    ended_at: String,
    location: String,
    meeting_link: String,
    description: String,
    note: String,
    recurrence_series_id: String,
    has_recurrence_rules: bool,
    is_all_day: bool,
    provider: String,
    participants_json: Option<String>,
}

fn str_field(row: &serde_json::Value, key: &str) -> String {
    row.get(key)
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string()
}

fn bool_field(row: &serde_json::Value, key: &str) -> bool {
    row.get(key)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn read_events_file(path: &Path) -> crate::Result<Vec<LegacyEvent>> {
    let content = std::fs::read_to_string(path)?;
    let Ok(table) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&content) else {
        return Ok(Vec::new());
    };

    let mut events = Vec::new();
    for (id, row) in table {
        let participants_json = row
            .get("participants")
            .filter(|value| !value.is_null())
            .map(|value| value.to_string());

        events.push(LegacyEvent {
            id,
            tracking_id_event: str_field(&row, "tracking_id_event"),
            calendar_id: str_field(&row, "calendar_id"),
            title: str_field(&row, "title"),
            started_at: str_field(&row, "started_at"),
            ended_at: str_field(&row, "ended_at"),
            location: str_field(&row, "location"),
            meeting_link: str_field(&row, "meeting_link"),
            description: str_field(&row, "description"),
            note: str_field(&row, "note"),
            recurrence_series_id: str_field(&row, "recurrence_series_id"),
            has_recurrence_rules: bool_field(&row, "has_recurrence_rules"),
            is_all_day: bool_field(&row, "is_all_day"),
            provider: str_field(&row, "provider"),
            participants_json,
        });
    }

    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core2::Db3;

    async fn test_db() -> Db3 {
        let db = Db3::connect_memory_plain().await.unwrap();
        hypr_db_app::migrate(db.pool()).await.unwrap();
        db
    }

    #[tokio::test]
    async fn import_legacy_events_from_path_imports_rows_and_serializes_participants() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.json");

        std::fs::write(
            &path,
            r#"{
              "evt-1": {
                "tracking_id_event": "track-1",
                "calendar_id": "cal-1",
                "title": "Standup",
                "started_at": "2026-04-15T09:00:00Z",
                "ended_at": "2026-04-15T09:30:00Z",
                "location": "",
                "meeting_link": "https://meet.example/1",
                "description": "Daily sync",
                "note": "",
                "recurrence_series_id": "series-1",
                "has_recurrence_rules": true,
                "is_all_day": false,
                "provider": "google",
                "participants": [{"email":"a@example.com"}]
              }
            }"#,
        )
        .unwrap();

        import_legacy_events_from_path(db.pool(), &path)
            .await
            .unwrap();

        let rows = hypr_db_app::list_events(db.pool()).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "evt-1");
        assert_eq!(
            rows[0].participants_json.as_deref(),
            Some(r#"[{"email":"a@example.com"}]"#)
        );
    }
}

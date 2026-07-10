use std::collections::HashMap;

use hypr_db_app::{LegacyEvent as ImportedEvent, LegacyImportBatch, LegacyImportRow};

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

pub(super) fn parse_legacy_events(content: &str) -> Result<LegacyImportBatch, String> {
    let table = serde_json::from_str::<HashMap<String, serde_json::Value>>(content)
        .map_err(|error| error.to_string())?;
    let rows = table
        .into_iter()
        .map(|(id, row)| {
            LegacyImportRow::Event(ImportedEvent {
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
                participants_json: row
                    .get("participants")
                    .filter(|value| !value.is_null())
                    .map(serde_json::Value::to_string),
            })
        })
        .collect();

    Ok(LegacyImportBatch {
        rows,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_legacy_events_preserves_participants() {
        let batch = parse_legacy_events(
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

        let LegacyImportRow::Event(event) = &batch.rows[0] else {
            panic!("expected event");
        };
        assert_eq!(event.id, "evt-1");
        assert_eq!(
            event.participants_json.as_deref(),
            Some(r#"[{"email":"a@example.com"}]"#)
        );
    }
}

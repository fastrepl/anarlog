use std::collections::HashMap;

use hypr_db_app::{LegacyCalendar as ImportedCalendar, LegacyImportBatch, LegacyImportRow};

pub(super) fn parse_legacy_calendars(content: &str) -> Result<LegacyImportBatch, String> {
    let table = serde_json::from_str::<HashMap<String, serde_json::Value>>(content)
        .map_err(|error| error.to_string())?;
    let rows = table
        .into_iter()
        .map(|(id, row)| {
            LegacyImportRow::Calendar(ImportedCalendar {
                id,
                tracking_id_calendar: row
                    .get("tracking_id_calendar")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: row
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
                enabled: row
                    .get("enabled")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                provider: row
                    .get("provider")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
                source: row
                    .get("source")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
                color: row
                    .get("color")
                    .and_then(|value| value.as_str())
                    .unwrap_or("#888")
                    .to_string(),
                connection_id: row
                    .get("connection_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
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
    fn parse_legacy_calendars_preserves_rows() {
        let batch = parse_legacy_calendars(
            r##"{
              "cal-1": {
                "tracking_id_calendar": "track-1",
                "name": "Work",
                "enabled": true,
                "provider": "google",
                "source": "team",
                "color": "#111111",
                "connection_id": "conn-1"
              }
            }"##,
        )
        .unwrap();

        let LegacyImportRow::Calendar(calendar) = &batch.rows[0] else {
            panic!("expected calendar");
        };
        assert_eq!(calendar.id, "cal-1");
        assert_eq!(calendar.name, "Work");
    }
}

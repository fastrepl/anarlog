use std::collections::HashMap;
use std::path::Path;

use hypr_db_app::UpsertCalendar;
use sqlx::SqlitePool;

pub async fn import_legacy_calendars_from_path(
    pool: &SqlitePool,
    path: &Path,
) -> crate::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let calendars = read_calendars_file(path)?;
    for cal in calendars {
        hypr_db_app::insert_calendar_if_missing(
            pool,
            UpsertCalendar {
                id: &cal.id,
                tracking_id_calendar: &cal.tracking_id_calendar,
                name: &cal.name,
                enabled: cal.enabled,
                provider: &cal.provider,
                source: &cal.source,
                color: &cal.color,
                connection_id: &cal.connection_id,
            },
        )
        .await?;
    }

    Ok(())
}

struct LegacyCalendar {
    id: String,
    tracking_id_calendar: String,
    name: String,
    enabled: bool,
    provider: String,
    source: String,
    color: String,
    connection_id: String,
}

fn read_calendars_file(path: &Path) -> crate::Result<Vec<LegacyCalendar>> {
    let content = std::fs::read_to_string(path)?;
    let Ok(table) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&content) else {
        return Ok(Vec::new());
    };

    let mut calendars = Vec::new();
    for (id, row) in table {
        calendars.push(LegacyCalendar {
            id,
            tracking_id_calendar: row
                .get("tracking_id_calendar")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            name: row
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            enabled: row
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            provider: row
                .get("provider")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            source: row
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            color: row
                .get("color")
                .and_then(|v| v.as_str())
                .unwrap_or("#888")
                .to_string(),
            connection_id: row
                .get("connection_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        });
    }

    Ok(calendars)
}

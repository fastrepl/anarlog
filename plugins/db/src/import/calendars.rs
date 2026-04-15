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
    for calendar in calendars {
        hypr_db_app::insert_calendar_if_missing(
            pool,
            UpsertCalendar {
                id: &calendar.id,
                tracking_id_calendar: &calendar.tracking_id_calendar,
                name: &calendar.name,
                enabled: calendar.enabled,
                provider: &calendar.provider,
                source: &calendar.source,
                color: &calendar.color,
                connection_id: &calendar.connection_id,
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
        });
    }

    Ok(calendars)
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
    async fn import_legacy_calendars_from_path_imports_rows() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("calendars.json");

        std::fs::write(
            &path,
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

        import_legacy_calendars_from_path(db.pool(), &path)
            .await
            .unwrap();

        let rows = hypr_db_app::list_calendars(db.pool()).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "cal-1");
        assert_eq!(rows[0].name, "Work");
    }
}

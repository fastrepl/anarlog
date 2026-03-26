use sqlx::{Row, SqlitePool};

use crate::CalendarRow;

pub async fn has_calendars(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM calendars) AS has_any")
        .fetch_one(pool)
        .await?;
    let has_any: bool = row.get("has_any");
    Ok(has_any)
}

pub async fn upsert_calendar(
    pool: &SqlitePool,
    id: &str,
    provider: &str,
    connection_id: &str,
    tracking_id: &str,
    name: &str,
    color: &str,
    source: &str,
    enabled: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR REPLACE INTO calendars (id, provider, connection_id, tracking_id, name, color, source, enabled) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(provider)
    .bind(connection_id)
    .bind(tracking_id)
    .bind(name)
    .bind(color)
    .bind(source)
    .bind(enabled as i32)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_enabled_calendars_by_provider(
    pool: &SqlitePool,
    provider: &str,
    connection_id: &str,
) -> Result<Vec<CalendarRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, i32, String, String, String)>(
        "SELECT id, provider, connection_id, tracking_id, name, color, source, enabled, created_at, user_id, raw_json FROM calendars WHERE provider = ? AND connection_id = ? AND enabled = 1 ORDER BY name",
    )
    .bind(provider)
    .bind(connection_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                provider,
                connection_id,
                tracking_id,
                name,
                color,
                source,
                enabled,
                created_at,
                user_id,
                raw_json,
            )| CalendarRow {
                id,
                provider,
                connection_id,
                tracking_id,
                name,
                color,
                source,
                enabled: enabled != 0,
                created_at,
                user_id,
                raw_json,
            },
        )
        .collect())
}

pub async fn list_all_calendars_by_provider(
    pool: &SqlitePool,
    provider: &str,
) -> Result<Vec<CalendarRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, i32, String, String, String)>(
        "SELECT id, provider, connection_id, tracking_id, name, color, source, enabled, created_at, user_id, raw_json FROM calendars WHERE provider = ? ORDER BY name",
    )
    .bind(provider)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                provider,
                connection_id,
                tracking_id,
                name,
                color,
                source,
                enabled,
                created_at,
                user_id,
                raw_json,
            )| CalendarRow {
                id,
                provider,
                connection_id,
                tracking_id,
                name,
                color,
                source,
                enabled: enabled != 0,
                created_at,
                user_id,
                raw_json,
            },
        )
        .collect())
}

pub async fn delete_calendar(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM calendars WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_calendars_by_connection(
    pool: &SqlitePool,
    connection_id: &str,
) -> Result<Vec<CalendarRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, i32, String, String, String)>(
        "SELECT id, provider, connection_id, tracking_id, name, color, source, enabled, created_at, user_id, raw_json FROM calendars WHERE connection_id = ? ORDER BY name",
    )
    .bind(connection_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                provider,
                connection_id,
                tracking_id,
                name,
                color,
                source,
                enabled,
                created_at,
                user_id,
                raw_json,
            )| CalendarRow {
                id,
                provider,
                connection_id,
                tracking_id,
                name,
                color,
                source,
                enabled: enabled != 0,
                created_at,
                user_id,
                raw_json,
            },
        )
        .collect())
}

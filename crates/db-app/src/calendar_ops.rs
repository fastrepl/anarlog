use sqlx::SqlitePool;

use crate::{CalendarRow, UpsertCalendar};

pub async fn get_calendar(pool: &SqlitePool, id: &str) -> Result<Option<CalendarRow>, sqlx::Error> {
    sqlx::query_as::<_, CalendarRow>("SELECT * FROM calendars WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn list_calendars(pool: &SqlitePool) -> Result<Vec<CalendarRow>, sqlx::Error> {
    sqlx::query_as::<_, CalendarRow>("SELECT * FROM calendars ORDER BY name")
        .fetch_all(pool)
        .await
}

pub async fn upsert_calendar(
    pool: &SqlitePool,
    input: UpsertCalendar<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO calendars \
         (id, tracking_id_calendar, name, enabled, provider, source, color, connection_id, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(id) DO UPDATE SET \
           tracking_id_calendar = excluded.tracking_id_calendar, \
           name = excluded.name, \
           enabled = excluded.enabled, \
           provider = excluded.provider, \
           source = excluded.source, \
           color = excluded.color, \
           connection_id = excluded.connection_id, \
           updated_at = excluded.updated_at",
    )
    .bind(input.id)
    .bind(input.tracking_id_calendar)
    .bind(input.name)
    .bind(input.enabled)
    .bind(input.provider)
    .bind(input.source)
    .bind(input.color)
    .bind(input.connection_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn insert_calendar_if_missing(
    pool: &SqlitePool,
    input: UpsertCalendar<'_>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO calendars \
         (id, tracking_id_calendar, name, enabled, provider, source, color, connection_id, \
          created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, \
          strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(id) DO NOTHING",
    )
    .bind(input.id)
    .bind(input.tracking_id_calendar)
    .bind(input.name)
    .bind(input.enabled)
    .bind(input.provider)
    .bind(input.source)
    .bind(input.color)
    .bind(input.connection_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete_calendar(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM calendars WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

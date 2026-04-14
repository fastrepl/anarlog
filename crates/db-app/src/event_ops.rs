use sqlx::SqlitePool;

use crate::{EventRow, UpsertEvent};

pub async fn get_event(pool: &SqlitePool, id: &str) -> Result<Option<EventRow>, sqlx::Error> {
    sqlx::query_as::<_, EventRow>("SELECT * FROM events WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn list_events(pool: &SqlitePool) -> Result<Vec<EventRow>, sqlx::Error> {
    sqlx::query_as::<_, EventRow>("SELECT * FROM events ORDER BY started_at")
        .fetch_all(pool)
        .await
}

pub async fn upsert_event(pool: &SqlitePool, input: UpsertEvent<'_>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO events \
         (id, tracking_id_event, calendar_id, title, started_at, ended_at, \
          location, meeting_link, description, note, recurrence_series_id, \
          has_recurrence_rules, is_all_day, provider, participants_json, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(id) DO UPDATE SET \
           tracking_id_event = excluded.tracking_id_event, \
           calendar_id = excluded.calendar_id, \
           title = excluded.title, \
           started_at = excluded.started_at, \
           ended_at = excluded.ended_at, \
           location = excluded.location, \
           meeting_link = excluded.meeting_link, \
           description = excluded.description, \
           note = excluded.note, \
           recurrence_series_id = excluded.recurrence_series_id, \
           has_recurrence_rules = excluded.has_recurrence_rules, \
           is_all_day = excluded.is_all_day, \
           provider = excluded.provider, \
           participants_json = excluded.participants_json, \
           updated_at = excluded.updated_at",
    )
    .bind(input.id)
    .bind(input.tracking_id_event)
    .bind(input.calendar_id)
    .bind(input.title)
    .bind(input.started_at)
    .bind(input.ended_at)
    .bind(input.location)
    .bind(input.meeting_link)
    .bind(input.description)
    .bind(input.note)
    .bind(input.recurrence_series_id)
    .bind(input.has_recurrence_rules)
    .bind(input.is_all_day)
    .bind(input.provider)
    .bind(input.participants_json)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn insert_event_if_missing(
    pool: &SqlitePool,
    input: UpsertEvent<'_>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO events \
         (id, tracking_id_event, calendar_id, title, started_at, ended_at, \
          location, meeting_link, description, note, recurrence_series_id, \
          has_recurrence_rules, is_all_day, provider, participants_json, \
          created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, \
          strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(id) DO NOTHING",
    )
    .bind(input.id)
    .bind(input.tracking_id_event)
    .bind(input.calendar_id)
    .bind(input.title)
    .bind(input.started_at)
    .bind(input.ended_at)
    .bind(input.location)
    .bind(input.meeting_link)
    .bind(input.description)
    .bind(input.note)
    .bind(input.recurrence_series_id)
    .bind(input.has_recurrence_rules)
    .bind(input.is_all_day)
    .bind(input.provider)
    .bind(input.participants_json)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete_event(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM events WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn delete_events_by_calendar_id(
    pool: &SqlitePool,
    calendar_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM events WHERE calendar_id = ?")
        .bind(calendar_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

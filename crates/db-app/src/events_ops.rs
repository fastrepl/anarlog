use sqlx::{Row, SqlitePool};

use crate::{EventParticipantRow, EventRow};

pub async fn upsert_event(
    pool: &SqlitePool,
    id: &str,
    user_id: &str,
    calendar_id: &str,
    tracking_id: &str,
    title: &str,
    started_at: &str,
    ended_at: &str,
    location: &str,
    meeting_link: &str,
    description: &str,
    note: &str,
    recurrence_series_id: &str,
    has_recurrence_rules: bool,
    is_all_day: bool,
    participants_json: &str,
    raw_json: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR REPLACE INTO events (id, user_id, calendar_id, tracking_id, title, started_at, ended_at, location, meeting_link, description, note, recurrence_series_id, has_recurrence_rules, is_all_day, participants_json, raw_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(user_id)
    .bind(calendar_id)
    .bind(tracking_id)
    .bind(title)
    .bind(started_at)
    .bind(ended_at)
    .bind(location)
    .bind(meeting_link)
    .bind(description)
    .bind(note)
    .bind(recurrence_series_id)
    .bind(has_recurrence_rules as i32)
    .bind(is_all_day as i32)
    .bind(participants_json)
    .bind(raw_json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_events_by_calendar(
    pool: &SqlitePool,
    calendar_id: &str,
) -> Result<Vec<EventRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, user_id, calendar_id, tracking_id, title, started_at, ended_at, location, meeting_link, description, note, recurrence_series_id, has_recurrence_rules, is_all_day, participants_json, raw_json, created_at FROM events WHERE calendar_id = ? ORDER BY started_at",
    )
    .bind(calendar_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(map_event_row).collect())
}

pub async fn list_events_in_range(
    pool: &SqlitePool,
    start: &str,
    end: &str,
) -> Result<Vec<EventRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, user_id, calendar_id, tracking_id, title, started_at, ended_at, location, meeting_link, description, note, recurrence_series_id, has_recurrence_rules, is_all_day, participants_json, raw_json, created_at FROM events WHERE started_at >= ? AND started_at < ? ORDER BY started_at",
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(map_event_row).collect())
}

pub async fn find_current_or_upcoming_event(
    pool: &SqlitePool,
    lookahead_minutes: i64,
) -> Result<Option<EventRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, user_id, calendar_id, tracking_id, title, started_at, ended_at, location, meeting_link, description, note, recurrence_series_id, has_recurrence_rules, is_all_day, participants_json, raw_json, created_at FROM events WHERE ended_at > datetime('now') AND started_at <= datetime('now', '+' || ? || ' minutes') ORDER BY started_at ASC LIMIT 1",
    )
    .bind(lookahead_minutes)
    .fetch_optional(pool)
    .await?;

    Ok(row.as_ref().map(map_event_row))
}

pub async fn list_events_by_calendar_ids(
    pool: &SqlitePool,
    calendar_ids: &[String],
    from: &str,
    to: &str,
) -> Result<Vec<EventRow>, sqlx::Error> {
    if calendar_ids.is_empty() {
        return Ok(vec![]);
    }
    let placeholders: Vec<&str> = calendar_ids.iter().map(|_| "?").collect();
    let query = format!(
        "SELECT id, user_id, calendar_id, tracking_id, title, started_at, ended_at, location, meeting_link, description, note, recurrence_series_id, has_recurrence_rules, is_all_day, participants_json, raw_json, created_at FROM events WHERE calendar_id IN ({}) AND started_at >= ? AND started_at < ? ORDER BY started_at",
        placeholders.join(", ")
    );
    let mut q = sqlx::query(&query);
    for id in calendar_ids {
        q = q.bind(id);
    }
    q = q.bind(from).bind(to);
    let rows = q.fetch_all(pool).await?;
    Ok(rows.iter().map(map_event_row).collect())
}

pub async fn delete_events_by_calendar(
    pool: &SqlitePool,
    calendar_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM events WHERE calendar_id = ?")
        .bind(calendar_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_event(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM events WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_event(pool: &SqlitePool, id: &str) -> Result<Option<EventRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, user_id, calendar_id, tracking_id, title, started_at, ended_at, location, meeting_link, description, note, recurrence_series_id, has_recurrence_rules, is_all_day, participants_json, raw_json, created_at FROM events WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(map_event_row))
}

fn map_event_row(row: &sqlx::sqlite::SqliteRow) -> EventRow {
    let has_recurrence_rules: i32 = row.get("has_recurrence_rules");
    let is_all_day: i32 = row.get("is_all_day");
    EventRow {
        id: row.get("id"),
        user_id: row.get("user_id"),
        calendar_id: row.get("calendar_id"),
        tracking_id: row.get("tracking_id"),
        title: row.get("title"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
        location: row.get("location"),
        meeting_link: row.get("meeting_link"),
        description: row.get("description"),
        note: row.get("note"),
        recurrence_series_id: row.get("recurrence_series_id"),
        has_recurrence_rules: has_recurrence_rules != 0,
        is_all_day: is_all_day != 0,
        participants_json: row.get("participants_json"),
        raw_json: row.get("raw_json"),
        created_at: row.get("created_at"),
    }
}

pub async fn upsert_event_participant(
    pool: &SqlitePool,
    id: &str,
    event_id: &str,
    human_id: Option<&str>,
    email: &str,
    name: &str,
    is_organizer: bool,
    is_current_user: bool,
    user_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR REPLACE INTO event_participants (id, event_id, human_id, email, name, is_organizer, is_current_user, user_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(event_id)
    .bind(human_id)
    .bind(email)
    .bind(name)
    .bind(is_organizer as i32)
    .bind(is_current_user as i32)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_event_participants(
    pool: &SqlitePool,
    event_id: &str,
) -> Result<Vec<EventParticipantRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, i32, i32, String)>(
        "SELECT id, event_id, human_id, email, name, is_organizer, is_current_user, user_id FROM event_participants WHERE event_id = ?",
    )
    .bind(event_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, event_id, human_id, email, name, is_organizer, is_current_user, user_id)| {
                EventParticipantRow {
                    id,
                    event_id,
                    human_id,
                    email,
                    name,
                    is_organizer: is_organizer != 0,
                    is_current_user: is_current_user != 0,
                    user_id,
                }
            },
        )
        .collect())
}

pub async fn list_events_by_human(
    pool: &SqlitePool,
    human_id: &str,
) -> Result<Vec<EventRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT e.id, e.user_id, e.calendar_id, e.tracking_id, e.title, e.started_at, e.ended_at, e.location, e.meeting_link, e.description, e.note, e.recurrence_series_id, e.has_recurrence_rules, e.is_all_day, e.participants_json, e.raw_json, e.created_at FROM events e JOIN event_participants ep ON e.id = ep.event_id WHERE ep.human_id = ? ORDER BY e.started_at DESC",
    )
    .bind(human_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(map_event_row).collect())
}

pub async fn list_events_by_org(
    pool: &SqlitePool,
    org_id: &str,
) -> Result<Vec<EventRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT DISTINCT e.id, e.user_id, e.calendar_id, e.tracking_id, e.title, e.started_at, e.ended_at, e.location, e.meeting_link, e.description, e.note, e.recurrence_series_id, e.has_recurrence_rules, e.is_all_day, e.participants_json, e.raw_json, e.created_at FROM events e JOIN event_participants ep ON e.id = ep.event_id JOIN humans h ON ep.human_id = h.id WHERE h.org_id = ? ORDER BY e.started_at DESC LIMIT 10",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(map_event_row).collect())
}

pub async fn link_event_participant_to_human(
    pool: &SqlitePool,
    event_id: &str,
    email: &str,
    human_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE event_participants SET human_id = ? WHERE event_id = ? AND email = ?")
        .bind(human_id)
        .bind(event_id)
        .bind(email)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_event_participants_by_event(
    pool: &SqlitePool,
    event_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM event_participants WHERE event_id = ?")
        .bind(event_id)
        .execute(pool)
        .await?;
    Ok(())
}

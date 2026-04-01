use sqlx::{Row, SqlitePool};

use crate::MeetingRow;

pub async fn get_meeting(pool: &SqlitePool, id: &str) -> Result<Option<MeetingRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, created_at, title, user_id, visibility, folder_id, event_id FROM meetings WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(|row| MeetingRow {
        id: row.get("id"),
        created_at: row.get("created_at"),
        title: row.get("title"),
        user_id: row.get("user_id"),
        visibility: row.get("visibility"),
        folder_id: row.get("folder_id"),
        event_id: row.get("event_id"),
    }))
}

pub async fn list_meetings(pool: &SqlitePool) -> Result<Vec<MeetingRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, created_at, title, user_id, visibility, folder_id, event_id FROM meetings ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|row| MeetingRow {
            id: row.get("id"),
            created_at: row.get("created_at"),
            title: row.get("title"),
            user_id: row.get("user_id"),
            visibility: row.get("visibility"),
            folder_id: row.get("folder_id"),
            event_id: row.get("event_id"),
        })
        .collect())
}

pub async fn update_meeting(
    pool: &SqlitePool,
    id: &str,
    title: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE meetings SET title = ? WHERE id = ?")
        .bind(title)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert_meeting(
    pool: &SqlitePool,
    meeting_id: &str,
    event_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT OR IGNORE INTO meetings (id, event_id) VALUES (?, ?)")
        .bind(meeting_id)
        .bind(event_id)
        .execute(pool)
        .await?;
    Ok(())
}

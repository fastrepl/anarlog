use sqlx::SqlitePool;

use crate::MeetingRow;

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

pub async fn update_meeting(
    pool: &SqlitePool,
    meeting_id: &str,
    title: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE meetings SET title = COALESCE(?, title) WHERE id = ?")
        .bind(title)
        .bind(meeting_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_meetings(pool: &SqlitePool) -> Result<Vec<MeetingRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, Option<String>, Option<String>)>(
        "SELECT id, created_at, title, user_id, visibility, folder_id, event_id FROM meetings ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, created_at, title, user_id, visibility, folder_id, event_id)| MeetingRow {
                id,
                created_at,
                title,
                user_id,
                visibility,
                folder_id,
                event_id,
            },
        )
        .collect())
}

pub async fn get_meeting(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<Option<MeetingRow>, sqlx::Error> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<String>,
            String,
            String,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT id, created_at, title, user_id, visibility, folder_id, event_id FROM meetings WHERE id = ?",
    )
    .bind(meeting_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, created_at, title, user_id, visibility, folder_id, event_id)| MeetingRow {
            id,
            created_at,
            title,
            user_id,
            visibility,
            folder_id,
            event_id,
        },
    ))
}

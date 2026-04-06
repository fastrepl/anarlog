use sqlx::SqlitePool;

use crate::MeetingParticipantRow;

pub async fn add_meeting_participant(
    pool: &SqlitePool,
    meeting_id: &str,
    human_id: &str,
    source: &str,
) -> Result<(), sqlx::Error> {
    let id = format!("{meeting_id}:{human_id}");
    sqlx::query(
        "INSERT OR REPLACE INTO meeting_participants (id, meeting_id, human_id, source) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(meeting_id)
    .bind(human_id)
    .bind(source)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn remove_meeting_participant(
    pool: &SqlitePool,
    meeting_id: &str,
    human_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM meeting_participants WHERE meeting_id = ? AND human_id = ?")
        .bind(meeting_id)
        .bind(human_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_meeting_participants(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<Vec<MeetingParticipantRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, meeting_id, human_id, source, user_id FROM meeting_participants WHERE meeting_id = ?",
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, meeting_id, human_id, source, user_id)| MeetingParticipantRow {
                id,
                meeting_id,
                human_id,
                source,
                user_id,
            },
        )
        .collect())
}

pub async fn copy_event_participants_to_meeting(
    pool: &SqlitePool,
    meeting_id: &str,
    event_id: &str,
) -> Result<usize, sqlx::Error> {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO meeting_participants (id, meeting_id, human_id, source) SELECT ? || ':' || human_id, ?, human_id, 'event' FROM event_participants WHERE event_id = ? AND human_id IS NOT NULL",
    )
    .bind(meeting_id)
    .bind(meeting_id)
    .bind(event_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() as usize)
}

pub async fn list_meetings_by_human(
    pool: &SqlitePool,
    human_id: &str,
) -> Result<Vec<MeetingParticipantRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, meeting_id, human_id, source, user_id FROM meeting_participants WHERE human_id = ?",
    )
    .bind(human_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, meeting_id, human_id, source, user_id)| MeetingParticipantRow {
                id,
                meeting_id,
                human_id,
                source,
                user_id,
            },
        )
        .collect())
}

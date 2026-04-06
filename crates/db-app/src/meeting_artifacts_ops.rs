use sqlx::{Row, SqlitePool};

use crate::{MeetingArtifactRow, MeetingSummaryRow};

pub async fn upsert_meeting_artifact(
    pool: &SqlitePool,
    id: &str,
    task_id: &str,
    transcript_md: &str,
    user_id: &str,
    visibility: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO meeting_artifacts (id, task_id, transcript_md, user_id, visibility, updated_at) VALUES (?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(task_id) DO UPDATE SET transcript_md = excluded.transcript_md, updated_at = excluded.updated_at",
    )
    .bind(id)
    .bind(task_id)
    .bind(transcript_md)
    .bind(user_id)
    .bind(visibility)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_meeting_artifact_by_task(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Option<MeetingArtifactRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, task_id, transcript_md, note_body, user_id, visibility, created_at, updated_at FROM meeting_artifacts WHERE task_id = ?",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(|row| MeetingArtifactRow {
        id: row.get("id"),
        task_id: row.get("task_id"),
        transcript_md: row.get("transcript_md"),
        note_body: row.get("note_body"),
        user_id: row.get("user_id"),
        visibility: row.get("visibility"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }))
}

pub async fn update_meeting_artifact_transcript(
    pool: &SqlitePool,
    id: &str,
    transcript_md: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE meeting_artifacts SET transcript_md = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(transcript_md)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_meeting_artifact_note_body(
    pool: &SqlitePool,
    id: &str,
    note_body: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE meeting_artifacts SET note_body = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(note_body)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_meeting_summary(
    pool: &SqlitePool,
    id: &str,
    task_id: &str,
    template_id: &str,
    title: &str,
    content: &str,
    position: i32,
    user_id: &str,
    visibility: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO meeting_summaries (id, task_id, template_id, title, content, position, user_id, visibility) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(task_id)
    .bind(template_id)
    .bind(title)
    .bind(content)
    .bind(position)
    .bind(user_id)
    .bind(visibility)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_meeting_summaries(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Vec<MeetingSummaryRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, task_id, template_id, content, position, title, user_id, visibility, created_at, updated_at FROM meeting_summaries WHERE task_id = ? ORDER BY position",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|row| MeetingSummaryRow {
            id: row.get("id"),
            task_id: row.get("task_id"),
            template_id: row.get("template_id"),
            content: row.get("content"),
            position: row.get("position"),
            title: row.get("title"),
            user_id: row.get("user_id"),
            visibility: row.get("visibility"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect())
}

pub async fn update_meeting_summary(
    pool: &SqlitePool,
    id: &str,
    content: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE meeting_summaries SET content = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(content)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_meeting_summaries_by_task(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM meeting_summaries WHERE task_id = ?")
        .bind(task_id)
        .execute(pool)
        .await?;
    Ok(())
}

use sqlx::SqlitePool;

use crate::NoteRow;

pub async fn insert_note(
    pool: &SqlitePool,
    id: &str,
    meeting_id: &str,
    kind: &str,
    title: &str,
    content: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO notes (id, meeting_id, kind, title, content) VALUES (?, ?, ?, ?, ?)")
        .bind(id)
        .bind(meeting_id)
        .bind(kind)
        .bind(title)
        .bind(content)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert_note_on_entity(
    pool: &SqlitePool,
    id: &str,
    entity_type: &str,
    entity_id: &str,
    kind: &str,
    title: &str,
    content: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO notes (id, meeting_id, kind, title, content, entity_type, entity_id) VALUES (?, '', ?, ?, ?, ?, ?)")
        .bind(id)
        .bind(kind)
        .bind(title)
        .bind(content)
        .bind(entity_type)
        .bind(entity_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_notes_by_meeting(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<Vec<NoteRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, String, String)>(
        "SELECT id, meeting_id, kind, title, content, created_at, user_id, visibility, entity_type, entity_id FROM notes WHERE meeting_id = ? ORDER BY created_at",
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                meeting_id,
                kind,
                title,
                content,
                created_at,
                user_id,
                visibility,
                entity_type,
                entity_id,
            )| NoteRow {
                id,
                meeting_id,
                kind,
                title,
                content,
                created_at,
                user_id,
                visibility,
                entity_type,
                entity_id,
            },
        )
        .collect())
}

pub async fn get_note_by_meeting_and_kind(
    pool: &SqlitePool,
    meeting_id: &str,
    kind: &str,
) -> Result<Option<NoteRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, String, String)>(
        "SELECT id, meeting_id, kind, title, content, created_at, user_id, visibility, entity_type, entity_id FROM notes WHERE meeting_id = ? AND kind = ? ORDER BY created_at DESC LIMIT 1",
    )
    .bind(meeting_id)
    .bind(kind)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(
            id,
            meeting_id,
            kind,
            title,
            content,
            created_at,
            user_id,
            visibility,
            entity_type,
            entity_id,
        )| NoteRow {
            id,
            meeting_id,
            kind,
            title,
            content,
            created_at,
            user_id,
            visibility,
            entity_type,
            entity_id,
        },
    ))
}

pub async fn update_note(pool: &SqlitePool, id: &str, content: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE notes SET content = ? WHERE id = ?")
        .bind(content)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_notes_by_meeting(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM notes WHERE meeting_id = ?")
        .bind(meeting_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_notes_by_entity(
    pool: &SqlitePool,
    entity_type: &str,
    entity_id: &str,
) -> Result<Vec<NoteRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, String, String)>(
        "SELECT id, meeting_id, kind, title, content, created_at, user_id, visibility, entity_type, entity_id FROM notes WHERE entity_type = ? AND entity_id = ? ORDER BY created_at",
    )
    .bind(entity_type)
    .bind(entity_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                meeting_id,
                kind,
                title,
                content,
                created_at,
                user_id,
                visibility,
                entity_type,
                entity_id,
            )| NoteRow {
                id,
                meeting_id,
                kind,
                title,
                content,
                created_at,
                user_id,
                visibility,
                entity_type,
                entity_id,
            },
        )
        .collect())
}

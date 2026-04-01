use sqlx::SqlitePool;

use crate::ChatMessageRow;

pub async fn insert_chat_message(
    pool: &SqlitePool,
    id: &str,
    meeting_id: &str,
    role: &str,
    content: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO chat_messages (id, meeting_id, role, content) VALUES (?, ?, ?, ?)")
        .bind(id)
        .bind(meeting_id)
        .bind(role)
        .bind(content)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn load_chat_messages(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<Vec<ChatMessageRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, meeting_id, role, content, created_at FROM chat_messages WHERE meeting_id = ? ORDER BY created_at",
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, meeting_id, role, content, created_at)| ChatMessageRow {
                id,
                meeting_id,
                role,
                content,
                created_at,
            },
        )
        .collect())
}

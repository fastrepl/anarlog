use sqlx::SqlitePool;

use crate::{MessageRow, ThreadRow};

pub async fn insert_thread(
    pool: &SqlitePool,
    id: &str,
    user_id: &str,
    meeting_id: Option<&str>,
    title: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO threads (id, user_id, meeting_id, title) VALUES (?, ?, ?, ?)")
        .bind(id)
        .bind(user_id)
        .bind(meeting_id)
        .bind(title)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_thread(pool: &SqlitePool, id: &str) -> Result<Option<ThreadRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, Option<String>, String, String, String)>(
        "SELECT id, user_id, meeting_id, title, visibility, created_at FROM threads WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, user_id, meeting_id, title, visibility, created_at)| ThreadRow {
            id,
            user_id,
            meeting_id,
            title,
            visibility,
            created_at,
        },
    ))
}

pub async fn list_threads_by_meeting(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<Vec<ThreadRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String)>(
        "SELECT id, user_id, meeting_id, title, visibility, created_at FROM threads WHERE meeting_id = ? ORDER BY created_at",
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, user_id, meeting_id, title, visibility, created_at)| ThreadRow {
                id,
                user_id,
                meeting_id,
                title,
                visibility,
                created_at,
            },
        )
        .collect())
}

pub async fn update_thread(
    pool: &SqlitePool,
    id: &str,
    title: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE threads SET title = COALESCE(?, title) WHERE id = ?")
        .bind(title)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_thread(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM messages WHERE thread_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM threads WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn insert_message(
    pool: &SqlitePool,
    id: &str,
    user_id: &str,
    thread_id: &str,
    role: &str,
    parts: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO messages (id, user_id, thread_id, role, parts) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(user_id)
    .bind(thread_id)
    .bind(role)
    .bind(parts)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn load_messages(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Vec<MessageRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT id, user_id, thread_id, role, parts, visibility, created_at FROM messages WHERE thread_id = ? ORDER BY created_at",
    )
    .bind(thread_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, user_id, thread_id, role, parts, visibility, created_at)| MessageRow {
                id,
                user_id,
                thread_id,
                role,
                parts,
                visibility,
                created_at,
            },
        )
        .collect())
}

pub async fn update_message(
    pool: &SqlitePool,
    id: &str,
    parts: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE messages SET parts = COALESCE(?, parts) WHERE id = ?")
        .bind(parts)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_messages_by_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM messages WHERE thread_id = ?")
        .bind(thread_id)
        .execute(pool)
        .await?;
    Ok(())
}

use sqlx::SqlitePool;

use crate::TaskNoteRow;

pub async fn insert_task_note(
    pool: &SqlitePool,
    id: &str,
    task_id: &str,
    author_type: &str,
    author_id: &str,
    body: &str,
    user_id: &str,
    visibility: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO task_notes (id, task_id, author_type, author_id, body, user_id, visibility) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(task_id)
    .bind(author_type)
    .bind(author_id)
    .bind(body)
    .bind(user_id)
    .bind(visibility)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_task_notes(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Vec<TaskNoteRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, Option<String>)>(
        "SELECT id, task_id, author_type, author_id, body, user_id, visibility, created_at, deleted_at FROM task_notes WHERE task_id = ? AND deleted_at IS NULL ORDER BY created_at",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                task_id,
                author_type,
                author_id,
                body,
                user_id,
                visibility,
                created_at,
                deleted_at,
            )| TaskNoteRow {
                id,
                task_id,
                author_type,
                author_id,
                body,
                user_id,
                visibility,
                created_at,
                deleted_at,
            },
        )
        .collect())
}

pub async fn soft_delete_task_note(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE task_notes SET deleted_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

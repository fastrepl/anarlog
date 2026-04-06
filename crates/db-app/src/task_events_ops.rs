use sqlx::SqlitePool;

use crate::TaskEventRow;

pub async fn insert_task_event(
    pool: &SqlitePool,
    id: &str,
    task_id: &str,
    actor_type: &str,
    actor_id: &str,
    event_type: &str,
    payload_json: &str,
    user_id: &str,
    visibility: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO task_events (id, task_id, actor_type, actor_id, event_type, payload_json, user_id, visibility) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(task_id)
    .bind(actor_type)
    .bind(actor_id)
    .bind(event_type)
    .bind(payload_json)
    .bind(user_id)
    .bind(visibility)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_task_events(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Vec<TaskEventRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, String)>(
        "SELECT id, task_id, actor_type, actor_id, event_type, payload_json, user_id, visibility, created_at FROM task_events WHERE task_id = ? ORDER BY created_at",
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
                actor_type,
                actor_id,
                event_type,
                payload_json,
                user_id,
                visibility,
                created_at,
            )| {
                TaskEventRow {
                    id,
                    task_id,
                    actor_type,
                    actor_id,
                    event_type,
                    payload_json,
                    user_id,
                    visibility,
                    created_at,
                }
            },
        )
        .collect())
}

pub async fn list_task_events_by_type(
    pool: &SqlitePool,
    task_id: &str,
    event_type: &str,
) -> Result<Vec<TaskEventRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, String)>(
        "SELECT id, task_id, actor_type, actor_id, event_type, payload_json, user_id, visibility, created_at FROM task_events WHERE task_id = ? AND event_type = ? ORDER BY created_at",
    )
    .bind(task_id)
    .bind(event_type)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                task_id,
                actor_type,
                actor_id,
                event_type,
                payload_json,
                user_id,
                visibility,
                created_at,
            )| {
                TaskEventRow {
                    id,
                    task_id,
                    actor_type,
                    actor_id,
                    event_type,
                    payload_json,
                    user_id,
                    visibility,
                    created_at,
                }
            },
        )
        .collect())
}

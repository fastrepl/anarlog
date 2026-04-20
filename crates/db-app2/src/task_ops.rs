use sqlx::SqlitePool;

use crate::{MoveTaskToSource, Task, UpsertTask};

pub async fn get_task(pool: &SqlitePool, id: &str) -> Result<Option<Task>, sqlx::Error> {
    sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn list_tasks_for_source(
    pool: &SqlitePool,
    source_type: &str,
    source_id: &str,
) -> Result<Vec<Task>, sqlx::Error> {
    sqlx::query_as::<_, Task>(
        "SELECT * FROM tasks \
         WHERE source_type = ? AND source_id = ? \
         ORDER BY source_order, id",
    )
    .bind(source_type)
    .bind(source_id)
    .fetch_all(pool)
    .await
}

pub async fn upsert_task(pool: &SqlitePool, input: UpsertTask<'_>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO tasks \
         (id, source_type, source_id, source_order, status, text_preview, body_json, due_date, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(id) DO UPDATE SET \
           source_type = excluded.source_type, \
           source_id = excluded.source_id, \
           source_order = excluded.source_order, \
           status = excluded.status, \
           text_preview = excluded.text_preview, \
           body_json = excluded.body_json, \
           due_date = excluded.due_date, \
           updated_at = excluded.updated_at",
    )
    .bind(input.id)
    .bind(input.source_type)
    .bind(input.source_id)
    .bind(input.source_order)
    .bind(input.status)
    .bind(input.text_preview)
    .bind(input.body_json)
    .bind(input.due_date)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_task(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM tasks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn move_tasks_to_source(
    pool: &SqlitePool,
    input: MoveTaskToSource<'_>,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    for (index, task_id) in input.task_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE tasks \
             SET source_type = ?, \
                 source_id = ?, \
                 source_order = ?, \
                 updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
             WHERE id = ?",
        )
        .bind(input.source_type)
        .bind(input.source_id)
        .bind(input.starting_source_order + index as i64)
        .bind(*task_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

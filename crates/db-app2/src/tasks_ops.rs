use sqlx::{Row, SqlitePool};

use crate::TaskRow;

const TASK_SELECT: &str = "SELECT id, daily_note_id, parent_task_id, sort_key, kind, title, status, body_json, source_type, source_id, due_date, metadata_json, user_id, created_at, updated_at FROM tasks";

pub async fn insert_task(
    pool: &SqlitePool,
    id: &str,
    daily_note_id: &str,
    kind: &str,
    title: &str,
    sort_key: &str,
    user_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO tasks (id, daily_note_id, kind, title, sort_key, user_id) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(daily_note_id)
    .bind(kind)
    .bind(title)
    .bind(sort_key)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_task(pool: &SqlitePool, id: &str) -> Result<Option<TaskRow>, sqlx::Error> {
    let row = sqlx::query(&format!("{TASK_SELECT} WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?;

    Ok(row.as_ref().map(map_task_row))
}

pub async fn list_tasks_by_daily_note(
    pool: &SqlitePool,
    daily_note_id: &str,
) -> Result<Vec<TaskRow>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "{TASK_SELECT} WHERE daily_note_id = ? ORDER BY sort_key"
    ))
    .bind(daily_note_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(map_task_row).collect())
}

pub async fn list_subtasks(
    pool: &SqlitePool,
    parent_task_id: &str,
) -> Result<Vec<TaskRow>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "{TASK_SELECT} WHERE parent_task_id = ? ORDER BY sort_key"
    ))
    .bind(parent_task_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(map_task_row).collect())
}

pub async fn update_task_title(
    pool: &SqlitePool,
    id: &str,
    title: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tasks \
         SET title = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?",
    )
    .bind(title)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_task_status(
    pool: &SqlitePool,
    id: &str,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tasks \
         SET status = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?",
    )
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_task_body(
    pool: &SqlitePool,
    id: &str,
    body_json: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tasks \
         SET body_json = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?",
    )
    .bind(body_json)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_task_sort_key(
    pool: &SqlitePool,
    id: &str,
    sort_key: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE tasks SET sort_key = ? WHERE id = ?")
        .bind(sort_key)
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn update_task_parent(
    pool: &SqlitePool,
    id: &str,
    parent_task_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE tasks SET parent_task_id = ? WHERE id = ?")
        .bind(parent_task_id)
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn update_task_metadata(
    pool: &SqlitePool,
    id: &str,
    metadata_json: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tasks \
         SET metadata_json = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?",
    )
    .bind(metadata_json)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_task_due_date(
    pool: &SqlitePool,
    id: &str,
    due_date: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tasks \
         SET due_date = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?",
    )
    .bind(due_date)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn reschedule_task(
    pool: &SqlitePool,
    id: &str,
    new_daily_note_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tasks \
         SET daily_note_id = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?",
    )
    .bind(new_daily_note_id)
    .bind(id)
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

pub async fn delete_task_cascade(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    let rows = sqlx::query_as::<_, (String,)>(
        "WITH RECURSIVE subtree(id) AS ( \
            SELECT id FROM tasks WHERE id = ? \
            UNION ALL \
            SELECT t.id FROM tasks t JOIN subtree s ON t.parent_task_id = s.id \
         ) \
         SELECT id FROM subtree",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;

    for (task_id,) in rows.into_iter().rev() {
        sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(())
}

fn map_task_row(row: &sqlx::sqlite::SqliteRow) -> TaskRow {
    TaskRow {
        id: row.get("id"),
        daily_note_id: row.get("daily_note_id"),
        parent_task_id: row.get("parent_task_id"),
        sort_key: row.get("sort_key"),
        kind: row.get("kind"),
        title: row.get("title"),
        status: row.get("status"),
        body_json: row.get("body_json"),
        source_type: row.get("source_type"),
        source_id: row.get("source_id"),
        due_date: row.get("due_date"),
        metadata_json: row.get("metadata_json"),
        user_id: row.get("user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

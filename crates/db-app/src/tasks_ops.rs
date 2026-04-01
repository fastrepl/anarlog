use sqlx::{Row, SqlitePool};

use crate::{TaskRow, TaskWithEventRow};

const TASK_SELECT: &str = "SELECT id, daily_id, parent_task_id, event_id, sort_key, type, title, status, source_id, source_url, metadata_json, user_id, visibility, created_at, updated_at, updated_by FROM tasks";

pub async fn insert_task(
    pool: &SqlitePool,
    id: &str,
    daily_id: &str,
    task_type: &str,
    title: &str,
    sort_key: &str,
    user_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO tasks (id, daily_id, type, title, sort_key, user_id) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(daily_id)
    .bind(task_type)
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

pub async fn get_task_by_event(
    pool: &SqlitePool,
    event_id: &str,
) -> Result<Option<TaskRow>, sqlx::Error> {
    let row = sqlx::query(&format!("{TASK_SELECT} WHERE event_id = ?"))
        .bind(event_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(map_task_row))
}

pub async fn list_tasks_by_daily(
    pool: &SqlitePool,
    daily_id: &str,
) -> Result<Vec<TaskRow>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "{TASK_SELECT} WHERE daily_id = ? ORDER BY sort_key"
    ))
    .bind(daily_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(map_task_row).collect())
}

pub async fn list_tasks_with_events_by_daily(
    pool: &SqlitePool,
    daily_id: &str,
) -> Result<Vec<TaskWithEventRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT \
            t.id, t.daily_id, t.parent_task_id, t.event_id, t.sort_key, t.type, t.title, t.status, t.source_id, t.source_url, t.metadata_json, t.user_id, t.visibility, t.created_at, t.updated_at, t.updated_by, \
            e.tracking_id AS event_tracking_id, e.started_at AS event_started_at, e.ended_at AS event_ended_at, e.is_all_day AS event_is_all_day, e.meeting_link AS event_meeting_link, e.location AS event_location, e.sync_status AS event_sync_status, e.deleted_at AS event_deleted_at \
         FROM tasks t \
         LEFT JOIN events e ON e.id = t.event_id \
         WHERE t.daily_id = ? \
         ORDER BY t.sort_key",
    )
    .bind(daily_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(map_task_with_event_row).collect())
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
        "UPDATE tasks SET title = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
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
    updated_by: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tasks SET status = ?, updated_by = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(status)
    .bind(updated_by)
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
        "UPDATE tasks SET metadata_json = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(metadata_json)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn reschedule_task(
    pool: &SqlitePool,
    id: &str,
    new_daily_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tasks SET daily_id = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(new_daily_id)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn link_task_to_event(
    pool: &SqlitePool,
    task_id: &str,
    event_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tasks SET event_id = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(event_id)
    .bind(task_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn unlink_task_from_event(pool: &SqlitePool, task_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tasks SET event_id = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(task_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn ensure_task_for_event(
    pool: &SqlitePool,
    task_id: &str,
    event_id: &str,
    daily_id: &str,
    sort_key: &str,
    user_id: &str,
) -> Result<String, sqlx::Error> {
    let row = sqlx::query("SELECT title FROM events WHERE id = ?")
        .bind(event_id)
        .fetch_optional(pool)
        .await?;
    let title = match row {
        Some(row) => row.get::<String, _>("title"),
        None => return Err(sqlx::Error::RowNotFound),
    };

    if let Some(existing) = get_task_by_event(pool, event_id).await? {
        sqlx::query(
            "UPDATE tasks \
             SET daily_id = ?, title = ?, sort_key = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), \
                 type = CASE WHEN type = 'meeting' THEN 'meeting' ELSE 'event' END \
             WHERE id = ?",
        )
        .bind(daily_id)
        .bind(title)
        .bind(sort_key)
        .bind(&existing.id)
        .execute(pool)
        .await?;
        return Ok(existing.id);
    }

    sqlx::query(
        "INSERT INTO tasks (id, daily_id, event_id, type, title, sort_key, user_id) VALUES (?, ?, ?, 'event', ?, ?, ?)",
    )
    .bind(task_id)
    .bind(daily_id)
    .bind(event_id)
    .bind(title)
    .bind(sort_key)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(task_id.to_string())
}

pub async fn promote_task_to_meeting(pool: &SqlitePool, task_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tasks SET type = 'meeting', updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(task_id)
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

    let task_ids: Vec<String> = rows.into_iter().map(|r| r.0).collect();
    if task_ids.is_empty() {
        tx.commit().await?;
        return Ok(());
    }

    for task_id in &task_ids {
        sqlx::query("DELETE FROM task_speaker_hints WHERE task_id = ?")
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM task_words WHERE task_id = ?")
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM task_participants WHERE task_id = ?")
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM task_notes WHERE task_id = ?")
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM task_events WHERE task_id = ?")
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM meeting_summaries WHERE task_id = ?")
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM meeting_artifacts WHERE task_id = ?")
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
    }

    for task_id in task_ids.iter().rev() {
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
        daily_id: row.get("daily_id"),
        parent_task_id: row.get("parent_task_id"),
        event_id: row.get("event_id"),
        sort_key: row.get("sort_key"),
        task_type: row.get("type"),
        title: row.get("title"),
        status: row.get("status"),
        source_id: row.get("source_id"),
        source_url: row.get("source_url"),
        metadata_json: row.get("metadata_json"),
        user_id: row.get("user_id"),
        visibility: row.get("visibility"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        updated_by: row.get("updated_by"),
    }
}

fn map_task_with_event_row(row: &sqlx::sqlite::SqliteRow) -> TaskWithEventRow {
    let event_is_all_day = row.get::<Option<i32>, _>("event_is_all_day");
    TaskWithEventRow {
        id: row.get("id"),
        daily_id: row.get("daily_id"),
        parent_task_id: row.get("parent_task_id"),
        event_id: row.get("event_id"),
        sort_key: row.get("sort_key"),
        task_type: row.get("type"),
        title: row.get("title"),
        status: row.get("status"),
        source_id: row.get("source_id"),
        source_url: row.get("source_url"),
        metadata_json: row.get("metadata_json"),
        user_id: row.get("user_id"),
        visibility: row.get("visibility"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        updated_by: row.get("updated_by"),
        event_tracking_id: row.get("event_tracking_id"),
        event_started_at: row.get("event_started_at"),
        event_ended_at: row.get("event_ended_at"),
        event_is_all_day: event_is_all_day.map(|v| v != 0),
        event_meeting_link: row.get("event_meeting_link"),
        event_location: row.get("event_location"),
        event_sync_status: row.get("event_sync_status"),
        event_deleted_at: row.get("event_deleted_at"),
    }
}

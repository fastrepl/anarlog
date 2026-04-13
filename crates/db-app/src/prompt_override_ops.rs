use sqlx::{Row, SqlitePool};

use crate::{PromptOverrideRow, UpsertPromptOverride};

pub async fn get_prompt_override(
    pool: &SqlitePool,
    task_type: &str,
) -> Result<Option<PromptOverrideRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT task_type, content, created_at, updated_at \
         FROM prompt_overrides WHERE task_type = ?",
    )
    .bind(task_type)
    .fetch_optional(pool)
    .await?;

    Ok(row.as_ref().map(map_prompt_override_row))
}

pub async fn list_prompt_overrides(
    pool: &SqlitePool,
) -> Result<Vec<PromptOverrideRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT task_type, content, created_at, updated_at \
         FROM prompt_overrides ORDER BY task_type",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(map_prompt_override_row).collect())
}

pub async fn upsert_prompt_override(
    pool: &SqlitePool,
    input: UpsertPromptOverride<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO prompt_overrides (task_type, content, updated_at) \
         VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(task_type) DO UPDATE SET \
           content = excluded.content, \
           updated_at = excluded.updated_at",
    )
    .bind(input.task_type)
    .bind(input.content)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_prompt_override(pool: &SqlitePool, task_type: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM prompt_overrides WHERE task_type = ?")
        .bind(task_type)
        .execute(pool)
        .await?;

    Ok(())
}

fn map_prompt_override_row(row: &sqlx::sqlite::SqliteRow) -> PromptOverrideRow {
    PromptOverrideRow {
        task_type: row.get("task_type"),
        content: row.get("content"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

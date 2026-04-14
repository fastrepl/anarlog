use sqlx::SqlitePool;

use crate::{TemplateRow, UpsertTemplate};

pub async fn get_template(pool: &SqlitePool, id: &str) -> Result<Option<TemplateRow>, sqlx::Error> {
    sqlx::query_as::<_, TemplateRow>("SELECT * FROM templates WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn list_templates(pool: &SqlitePool) -> Result<Vec<TemplateRow>, sqlx::Error> {
    sqlx::query_as::<_, TemplateRow>("SELECT * FROM templates ORDER BY id")
        .fetch_all(pool)
        .await
}

pub async fn upsert_template(
    pool: &SqlitePool,
    input: UpsertTemplate<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO templates \
         (id, title, description, pinned, pin_order, category, targets_json, sections_json, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(id) DO UPDATE SET \
           title = excluded.title, \
           description = excluded.description, \
           pinned = excluded.pinned, \
           pin_order = excluded.pin_order, \
           category = excluded.category, \
           targets_json = excluded.targets_json, \
           sections_json = excluded.sections_json, \
           updated_at = excluded.updated_at",
    )
    .bind(input.id)
    .bind(input.title)
    .bind(input.description)
    .bind(input.pinned)
    .bind(input.pin_order)
    .bind(input.category)
    .bind(input.targets_json)
    .bind(input.sections_json)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn insert_template_if_missing(
    pool: &SqlitePool,
    input: UpsertTemplate<'_>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO templates \
         (id, title, description, pinned, pin_order, category, targets_json, sections_json, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(id) DO NOTHING",
    )
    .bind(input.id)
    .bind(input.title)
    .bind(input.description)
    .bind(input.pinned)
    .bind(input.pin_order)
    .bind(input.category)
    .bind(input.targets_json)
    .bind(input.sections_json)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete_template(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM templates WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

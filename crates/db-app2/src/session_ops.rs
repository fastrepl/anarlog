use sqlx::SqlitePool;

use crate::{Session, UpsertSession};

pub async fn get_session(pool: &SqlitePool, id: &str) -> Result<Option<Session>, sqlx::Error> {
    sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn list_sessions(pool: &SqlitePool) -> Result<Vec<Session>, sqlx::Error> {
    sqlx::query_as::<_, Session>(
        "SELECT * FROM sessions ORDER BY datetime(updated_at) DESC, id DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn upsert_session(
    pool: &SqlitePool,
    input: UpsertSession<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO sessions \
         (id, title, raw_md, folder_id, event_json, updated_at) \
         VALUES (?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(id) DO UPDATE SET \
           title = excluded.title, \
           raw_md = excluded.raw_md, \
           folder_id = excluded.folder_id, \
           event_json = excluded.event_json, \
           updated_at = excluded.updated_at",
    )
    .bind(input.id)
    .bind(input.title)
    .bind(input.raw_md)
    .bind(input.folder_id)
    .bind(input.event_json)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn insert_session_if_missing(
    pool: &SqlitePool,
    input: UpsertSession<'_>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO sessions \
         (id, title, raw_md, folder_id, event_json, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(id) DO NOTHING",
    )
    .bind(input.id)
    .bind(input.title)
    .bind(input.raw_md)
    .bind(input.folder_id)
    .bind(input.event_json)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete_session(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

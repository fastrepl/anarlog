use sqlx::{Row, SqlitePool};

use crate::UserRow;

pub async fn insert_user(pool: &SqlitePool, id: &str, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO users (id, name) VALUES (?, ?)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_user(pool: &SqlitePool, id: &str) -> Result<Option<UserRow>, sqlx::Error> {
    let row = sqlx::query("SELECT id, name, created_at FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(|row| UserRow {
        id: row.get("id"),
        name: row.get("name"),
        created_at: row.get("created_at"),
    }))
}

pub async fn update_user(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET name = COALESCE(?, name) WHERE id = ?")
        .bind(name)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

use sqlx::SqlitePool;

use crate::FolderRow;

pub async fn insert_folder(
    pool: &SqlitePool,
    id: &str,
    user_id: &str,
    name: &str,
    parent_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO folders (id, user_id, name, parent_id) VALUES (?, ?, ?, ?)")
        .bind(id)
        .bind(user_id)
        .bind(name)
        .bind(parent_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_folder(pool: &SqlitePool, id: &str) -> Result<Option<FolderRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, String, Option<String>, String)>(
        "SELECT id, user_id, name, parent_id, created_at FROM folders WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(
        row.map(|(id, user_id, name, parent_id, created_at)| FolderRow {
            id,
            user_id,
            name,
            parent_id,
            created_at,
        }),
    )
}

pub async fn list_folders(pool: &SqlitePool) -> Result<Vec<FolderRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, String)>(
        "SELECT id, user_id, name, parent_id, created_at FROM folders ORDER BY name",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, user_id, name, parent_id, created_at)| FolderRow {
            id,
            user_id,
            name,
            parent_id,
            created_at,
        })
        .collect())
}

pub async fn update_folder(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
    parent_id: Option<Option<&str>>,
) -> Result<(), sqlx::Error> {
    match parent_id {
        Some(pid) => {
            sqlx::query("UPDATE folders SET name = COALESCE(?, name), parent_id = ? WHERE id = ?")
                .bind(name)
                .bind(pid)
                .bind(id)
                .execute(pool)
                .await?;
        }
        None => {
            sqlx::query("UPDATE folders SET name = COALESCE(?, name) WHERE id = ?")
                .bind(name)
                .bind(id)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

pub async fn delete_folder(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE meetings SET folder_id = NULL WHERE folder_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE folders SET parent_id = NULL WHERE parent_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM folders WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

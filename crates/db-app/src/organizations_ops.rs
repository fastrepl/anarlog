use sqlx::SqlitePool;

use crate::OrganizationRow;

pub async fn insert_organization(
    pool: &SqlitePool,
    id: &str,
    name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO organizations (id, name) VALUES (?, ?)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_organization(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE organizations SET name = COALESCE(?, name) WHERE id = ?")
        .bind(name)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_organization(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<OrganizationRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, String, i32, i32, String)>(
        "SELECT id, created_at, name, pinned, pin_order, user_id FROM organizations WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, created_at, name, pinned, pin_order, user_id)| OrganizationRow {
            id,
            created_at,
            name,
            pinned: pinned != 0,
            pin_order,
            user_id,
        },
    ))
}

pub async fn list_organizations(pool: &SqlitePool) -> Result<Vec<OrganizationRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, i32, i32, String)>(
        "SELECT id, created_at, name, pinned, pin_order, user_id FROM organizations ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, created_at, name, pinned, pin_order, user_id)| OrganizationRow {
                id,
                created_at,
                name,
                pinned: pinned != 0,
                pin_order,
                user_id,
            },
        )
        .collect())
}

pub async fn delete_organization(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE humans SET org_id = '' WHERE org_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM organizations WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

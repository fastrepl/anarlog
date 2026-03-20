use sqlx::SqlitePool;

use crate::HumanRow;

pub async fn insert_human(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    email: &str,
    org_id: &str,
    job_title: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO humans (id, name, email, org_id, job_title) VALUES (?, ?, ?, ?, ?)")
        .bind(id)
        .bind(name)
        .bind(email)
        .bind(org_id)
        .bind(job_title)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_human(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
    email: Option<&str>,
    org_id: Option<&str>,
    job_title: Option<&str>,
    memo: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE humans SET name = COALESCE(?, name), email = COALESCE(?, email), org_id = COALESCE(?, org_id), job_title = COALESCE(?, job_title), memo = COALESCE(?, memo) WHERE id = ?",
    )
    .bind(name)
    .bind(email)
    .bind(org_id)
    .bind(job_title)
    .bind(memo)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_human(pool: &SqlitePool, id: &str) -> Result<Option<HumanRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, i32, i32, String, Option<String>)>(
        "SELECT id, created_at, name, email, org_id, job_title, linkedin_username, memo, pinned, pin_order, user_id, linked_user_id FROM humans WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(
            id,
            created_at,
            name,
            email,
            org_id,
            job_title,
            linkedin_username,
            memo,
            pinned,
            pin_order,
            user_id,
            linked_user_id,
        )| HumanRow {
            id,
            created_at,
            name,
            email,
            org_id,
            job_title,
            linkedin_username,
            memo,
            pinned: pinned != 0,
            pin_order,
            user_id,
            linked_user_id,
        },
    ))
}

pub async fn list_humans(pool: &SqlitePool) -> Result<Vec<HumanRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, i32, i32, String, Option<String>)>(
        "SELECT id, created_at, name, email, org_id, job_title, linkedin_username, memo, pinned, pin_order, user_id, linked_user_id FROM humans ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                created_at,
                name,
                email,
                org_id,
                job_title,
                linkedin_username,
                memo,
                pinned,
                pin_order,
                user_id,
                linked_user_id,
            )| HumanRow {
                id,
                created_at,
                name,
                email,
                org_id,
                job_title,
                linkedin_username,
                memo,
                pinned: pinned != 0,
                pin_order,
                user_id,
                linked_user_id,
            },
        )
        .collect())
}

pub async fn list_humans_by_org(
    pool: &SqlitePool,
    org_id: &str,
) -> Result<Vec<HumanRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, i32, i32, String, Option<String>)>(
        "SELECT id, created_at, name, email, org_id, job_title, linkedin_username, memo, pinned, pin_order, user_id, linked_user_id FROM humans WHERE org_id = ? ORDER BY created_at DESC",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                created_at,
                name,
                email,
                org_id,
                job_title,
                linkedin_username,
                memo,
                pinned,
                pin_order,
                user_id,
                linked_user_id,
            )| HumanRow {
                id,
                created_at,
                name,
                email,
                org_id,
                job_title,
                linkedin_username,
                memo,
                pinned: pinned != 0,
                pin_order,
                user_id,
                linked_user_id,
            },
        )
        .collect())
}

pub async fn delete_human(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM meeting_participants WHERE human_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM humans WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

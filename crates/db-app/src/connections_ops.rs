use sqlx::SqlitePool;

use crate::ConnectionRow;

pub async fn upsert_connection(
    pool: &SqlitePool,
    provider_type: &str,
    provider_id: &str,
    base_url: &str,
    api_key: &str,
) -> Result<(), sqlx::Error> {
    let id = format!("{provider_type}:{provider_id}");
    sqlx::query(
        "INSERT OR REPLACE INTO connections (id, provider_type, provider_id, base_url, api_key) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(provider_type)
    .bind(provider_id)
    .bind(base_url)
    .bind(api_key)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_connection(
    pool: &SqlitePool,
    provider_type: &str,
    provider_id: &str,
) -> Result<Option<ConnectionRow>, sqlx::Error> {
    let id = format!("{provider_type}:{provider_id}");
    let row = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT id, provider_type, provider_id, base_url, api_key, user_id FROM connections WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, provider_type, provider_id, base_url, api_key, user_id)| ConnectionRow {
            id,
            provider_type,
            provider_id,
            base_url,
            api_key,
            user_id,
        },
    ))
}

pub async fn list_connections(
    pool: &SqlitePool,
    provider_type: &str,
) -> Result<Vec<ConnectionRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT id, provider_type, provider_id, base_url, api_key, user_id FROM connections WHERE provider_type = ? ORDER BY provider_id",
    )
    .bind(provider_type)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, provider_type, provider_id, base_url, api_key, user_id)| ConnectionRow {
                id,
                provider_type,
                provider_id,
                base_url,
                api_key,
                user_id,
            },
        )
        .collect())
}

pub async fn list_configured_provider_ids(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String,)>("SELECT DISTINCT provider_id FROM connections")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

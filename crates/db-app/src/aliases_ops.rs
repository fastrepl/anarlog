use sqlx::SqlitePool;

use crate::{AliasRow, insert_human};

pub async fn upsert_alias(
    pool: &SqlitePool,
    id: &str,
    human_id: &str,
    provider: &str,
    external_id: &str,
    workspace_id: &str,
    display_name: &str,
    confidence: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR REPLACE INTO aliases (id, human_id, provider, external_id, workspace_id, display_name, confidence) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(human_id)
    .bind(provider)
    .bind(external_id)
    .bind(workspace_id)
    .bind(display_name)
    .bind(confidence)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_alias_by_external(
    pool: &SqlitePool,
    provider: &str,
    external_id: &str,
    workspace_id: &str,
) -> Result<Option<AliasRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, String)>(
        "SELECT id, human_id, provider, external_id, workspace_id, display_name, confidence, created_at, user_id FROM aliases WHERE provider = ? AND external_id = ? AND workspace_id = ?",
    )
    .bind(provider)
    .bind(external_id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(
            id,
            human_id,
            provider,
            external_id,
            workspace_id,
            display_name,
            confidence,
            created_at,
            user_id,
        )| AliasRow {
            id,
            human_id,
            provider,
            external_id,
            workspace_id,
            display_name,
            confidence,
            created_at,
            user_id,
        },
    ))
}

pub async fn list_aliases_by_human(
    pool: &SqlitePool,
    human_id: &str,
) -> Result<Vec<AliasRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, String)>(
        "SELECT id, human_id, provider, external_id, workspace_id, display_name, confidence, created_at, user_id FROM aliases WHERE human_id = ? ORDER BY created_at",
    )
    .bind(human_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                human_id,
                provider,
                external_id,
                workspace_id,
                display_name,
                confidence,
                created_at,
                user_id,
            )| AliasRow {
                id,
                human_id,
                provider,
                external_id,
                workspace_id,
                display_name,
                confidence,
                created_at,
                user_id,
            },
        )
        .collect())
}

pub async fn delete_alias(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM aliases WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn resolve_or_create_alias(
    pool: &SqlitePool,
    provider: &str,
    external_id: &str,
    workspace_id: &str,
    display_name: &str,
) -> Result<AliasRow, sqlx::Error> {
    if let Some(existing) = get_alias_by_external(pool, provider, external_id, workspace_id).await?
    {
        return Ok(existing);
    }

    let human_id = format!("human:{provider}:{external_id}");
    insert_human(pool, &human_id, display_name, "", "", "").await?;

    let alias_id = format!("alias:{provider}:{external_id}:{workspace_id}");
    upsert_alias(
        pool,
        &alias_id,
        &human_id,
        provider,
        external_id,
        workspace_id,
        display_name,
        "auto",
    )
    .await?;

    Ok(AliasRow {
        id: alias_id,
        human_id,
        provider: provider.to_string(),
        external_id: external_id.to_string(),
        workspace_id: workspace_id.to_string(),
        display_name: display_name.to_string(),
        confidence: "auto".to_string(),
        created_at: String::new(),
        user_id: String::new(),
    })
}

use sqlx::SqlitePool;

use crate::WebhookEndpointRow;

pub const WEBHOOK_SECRET_PREFIX: &str = "whsec_";

pub fn generate_webhook_secret() -> String {
    format!("{WEBHOOK_SECRET_PREFIX}{}", uuid::Uuid::new_v4().simple())
}

pub async fn insert_webhook_endpoint(
    pool: &SqlitePool,
    id: &str,
    url: &str,
    secret: &str,
    events_json: &str,
) -> Result<WebhookEndpointRow, sqlx::Error> {
    sqlx::query_as::<_, WebhookEndpointRow>(
        "INSERT INTO webhook_endpoints (id, url, secret, events_json) \
         VALUES (?, ?, ?, ?) RETURNING *",
    )
    .bind(id)
    .bind(url)
    .bind(secret)
    .bind(events_json)
    .fetch_one(pool)
    .await
}

pub async fn list_webhook_endpoints(
    pool: &SqlitePool,
) -> Result<Vec<WebhookEndpointRow>, sqlx::Error> {
    sqlx::query_as::<_, WebhookEndpointRow>(
        "SELECT * FROM webhook_endpoints ORDER BY created_at DESC, id DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_webhook_endpoint(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<WebhookEndpointRow>, sqlx::Error> {
    sqlx::query_as::<_, WebhookEndpointRow>("SELECT * FROM webhook_endpoints WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn set_webhook_endpoint_active(
    pool: &SqlitePool,
    id: &str,
    active: bool,
) -> Result<Option<WebhookEndpointRow>, sqlx::Error> {
    sqlx::query_as::<_, WebhookEndpointRow>(
        "UPDATE webhook_endpoints SET active = ? WHERE id = ? RETURNING *",
    )
    .bind(active)
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_active_webhook_endpoints(
    pool: &SqlitePool,
) -> Result<Vec<WebhookEndpointRow>, sqlx::Error> {
    sqlx::query_as::<_, WebhookEndpointRow>(
        "SELECT * FROM webhook_endpoints WHERE active = 1 ORDER BY created_at DESC, id DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn delete_webhook_endpoint(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM webhook_endpoints WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn record_webhook_delivery(
    pool: &SqlitePool,
    id: &str,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE webhook_endpoints \
         SET last_delivery_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), last_delivery_status = ? \
         WHERE id = ?",
    )
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> anlg_db_core::Db {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        crate::prepare_schema(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn webhook_endpoint_lifecycle_covers_create_deliver_delete() {
        let db = test_db().await;
        let secret = generate_webhook_secret();
        assert!(secret.starts_with(WEBHOOK_SECRET_PREFIX));

        let created = insert_webhook_endpoint(
            db.pool(),
            "webhook-1",
            "https://example.com/hooks",
            &secret,
            "[\"note.enhanced\"]",
        )
        .await
        .unwrap();
        assert!(created.active);
        assert_eq!(created.last_delivery_status, "");

        record_webhook_delivery(db.pool(), "webhook-1", "200 OK")
            .await
            .unwrap();
        let listed = list_active_webhook_endpoints(db.pool()).await.unwrap();
        assert_eq!(listed[0].last_delivery_status, "200 OK");
        assert!(listed[0].last_delivery_at.is_some());

        assert!(
            delete_webhook_endpoint(db.pool(), "webhook-1")
                .await
                .unwrap()
        );
        assert!(list_webhook_endpoints(db.pool()).await.unwrap().is_empty());
    }
}

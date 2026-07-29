use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::{ApiKeyRow, GeneratedApiKey, WebhookEndpointRow};

pub const API_KEY_PREFIX: &str = "anl_";
pub const WEBHOOK_SECRET_PREFIX: &str = "whsec_";

pub fn hash_api_key(key: &str) -> String {
    Sha256::digest(key.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn generate_api_key() -> GeneratedApiKey {
    let key = format!(
        "{API_KEY_PREFIX}{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    GeneratedApiKey {
        key_hash: hash_api_key(&key),
        key_prefix: key[..12.min(key.len())].to_string(),
        key,
    }
}

pub fn generate_webhook_secret() -> String {
    format!("{WEBHOOK_SECRET_PREFIX}{}", uuid::Uuid::new_v4().simple())
}

pub async fn insert_api_key(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    key_hash: &str,
    key_prefix: &str,
) -> Result<ApiKeyRow, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>(
        "INSERT INTO api_keys (id, name, key_hash, key_prefix) VALUES (?, ?, ?, ?) RETURNING *",
    )
    .bind(id)
    .bind(name)
    .bind(key_hash)
    .bind(key_prefix)
    .fetch_one(pool)
    .await
}

pub async fn list_api_keys(pool: &SqlitePool) -> Result<Vec<ApiKeyRow>, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>(
        "SELECT * FROM api_keys WHERE revoked_at IS NULL ORDER BY created_at DESC, id DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn find_active_api_key_by_hash(
    pool: &SqlitePool,
    key_hash: &str,
) -> Result<Option<ApiKeyRow>, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>(
        "SELECT * FROM api_keys WHERE key_hash = ? AND revoked_at IS NULL",
    )
    .bind(key_hash)
    .fetch_optional(pool)
    .await
}

pub async fn touch_api_key(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE api_keys SET last_used_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn revoke_api_key(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE api_keys SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND revoked_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
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

    #[test]
    fn generated_keys_hash_and_prefix_consistently() {
        let generated = generate_api_key();
        assert!(generated.key.starts_with(API_KEY_PREFIX));
        assert_eq!(generated.key_hash, hash_api_key(&generated.key));
        assert_eq!(generated.key_prefix, &generated.key[..12]);
        assert_eq!(generated.key_hash.len(), 64);
        assert_ne!(generate_api_key().key, generated.key);
    }

    #[tokio::test]
    async fn api_key_lifecycle_covers_create_verify_touch_revoke() {
        let db = test_db().await;
        let generated = generate_api_key();
        let created = insert_api_key(
            db.pool(),
            "key-1",
            "Zapier",
            &generated.key_hash,
            &generated.key_prefix,
        )
        .await
        .unwrap();
        assert_eq!(created.name, "Zapier");
        assert!(created.last_used_at.is_none());

        let found = find_active_api_key_by_hash(db.pool(), &generated.key_hash)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, "key-1");

        touch_api_key(db.pool(), "key-1").await.unwrap();
        let keys = list_api_keys(db.pool()).await.unwrap();
        assert!(keys[0].last_used_at.is_some());

        assert!(revoke_api_key(db.pool(), "key-1").await.unwrap());
        assert!(!revoke_api_key(db.pool(), "key-1").await.unwrap());
        assert!(
            find_active_api_key_by_hash(db.pool(), &generated.key_hash)
                .await
                .unwrap()
                .is_none()
        );
        assert!(list_api_keys(db.pool()).await.unwrap().is_empty());
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

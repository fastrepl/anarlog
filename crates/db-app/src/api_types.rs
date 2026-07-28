#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ApiKeyRow {
    pub id: String,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct WebhookEndpointRow {
    pub id: String,
    pub url: String,
    pub secret: String,
    pub events_json: String,
    pub active: bool,
    pub created_at: String,
    pub last_delivery_at: Option<String>,
    pub last_delivery_status: String,
}

pub struct GeneratedApiKey {
    pub key: String,
    pub key_hash: String,
    pub key_prefix: String,
}

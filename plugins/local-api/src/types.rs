use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub struct LocalApiStatus {
    pub enabled: bool,
    pub port: u16,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub struct ApiKeyInfo {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

impl From<hypr_db_app::ApiKeyRow> for ApiKeyInfo {
    fn from(row: hypr_db_app::ApiKeyRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            key_prefix: row.key_prefix,
            created_at: row.created_at,
            last_used_at: row.last_used_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub struct CreatedApiKey {
    #[serde(flatten)]
    pub info: ApiKeyInfo,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub struct WebhookInfo {
    pub id: String,
    pub url: String,
    pub events: Vec<String>,
    pub active: bool,
    pub created_at: String,
    pub last_delivery_at: Option<String>,
    pub last_delivery_status: String,
}

impl From<hypr_db_app::WebhookEndpointRow> for WebhookInfo {
    fn from(row: hypr_db_app::WebhookEndpointRow) -> Self {
        Self {
            id: row.id,
            url: row.url,
            events: serde_json::from_str(&row.events_json).unwrap_or_default(),
            active: row.active,
            created_at: row.created_at,
            last_delivery_at: row.last_delivery_at,
            last_delivery_status: row.last_delivery_status,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub struct CreatedWebhook {
    #[serde(flatten)]
    pub info: WebhookInfo,
    pub secret: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub struct WebhookDelivery {
    pub delivered: bool,
    pub status: String,
}

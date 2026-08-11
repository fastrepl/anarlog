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

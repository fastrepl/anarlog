use serde::{Deserialize, Serialize};
use utoipa::{
    Modify, OpenApi, ToSchema,
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
};

// Core webhook event structure
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebhookEvent {
    /// Unique event identifier
    #[schema(example = "evt_01234567890")]
    pub id: String,

    /// Event type
    #[schema(example = "note.created")]
    pub event_type: String,

    /// ISO 8601 timestamp
    #[schema(example = "2024-01-10T10:30:00Z")]
    pub timestamp: String,

    /// Event payload
    pub data: serde_json::Value,
}

// Simplified event payloads
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NoteEvent {
    #[schema(example = "note_abc123")]
    pub note_id: String,

    #[schema(example = "Meeting Notes")]
    pub title: String,

    #[schema(example = "Discussion points...")]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RecordingEvent {
    #[schema(example = "rec_xyz789")]
    pub recording_id: String,

    #[schema(example = 300)]
    pub duration_seconds: u32,

    #[schema(example = "completed")]
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TranscriptionEvent {
    #[schema(example = "rec_xyz789")]
    pub recording_id: String,

    #[schema(example = "trans_qwe456")]
    pub transcription_id: String,

    #[schema(example = "Hello, this is the transcribed text.")]
    pub text: String,
}

// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebhookConfig {
    /// Your webhook endpoint URL
    #[schema(example = "https://your-app.com/webhooks")]
    pub url: String,

    /// Events to subscribe to
    #[schema(example = json!(["note.created", "note.updated", "recording.completed"]))]
    pub events: Vec<String>,

    /// Whether the webhook is active
    #[schema(example = true)]
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateWebhookRequest {
    pub config: WebhookConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebhookResponse {
    #[schema(example = "webhook_123")]
    pub id: String,

    pub config: WebhookConfig,

    /// Secret for verifying webhook signatures
    #[schema(example = "whsec_1234567890abcdef")]
    pub secret: String,

    #[schema(example = "2024-01-10T10:00:00Z")]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebhookListResponse {
    pub webhooks: Vec<WebhookResponse>,
    pub total: usize,
}

// Webhook verification example
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebhookVerification {
    /// HMAC-SHA256 signature
    #[schema(example = "sha256=abcdef1234567890")]
    pub signature: String,

    /// Unix timestamp
    #[schema(example = "1704880200")]
    pub timestamp: String,
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Webhook API Documentation",
        version = "1.0.0",
        description = "Webhook system for receiving real-time events",
        contact(
            name = "API Support",
            email = "api@example.com"
        )
    ),
    paths(
        create_webhook,
        list_webhooks,
        delete_webhook,
        test_webhook,
        webhook_receiver_example
    ),
    components(
        schemas(
            WebhookEvent,
            NoteEvent,
            RecordingEvent,
            TranscriptionEvent,
            WebhookConfig,
            CreateWebhookRequest,
            WebhookResponse,
            WebhookListResponse,
            WebhookVerification
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "Webhooks", description = "Webhook management endpoints"),
        (name = "Events", description = "Event types and payloads")
    )
)]
pub struct ApiDoc;

#[utoipa::path(
    post,
    path = "/api/webhooks",
    tag = "Webhooks",
    request_body = CreateWebhookRequest,
    responses(
        (status = 201, description = "Webhook created", body = WebhookResponse),
        (status = 400, description = "Invalid configuration")
    ),
    security(
        ("api_key" = [])
    )
)]
#[allow(dead_code)]
async fn create_webhook() -> WebhookResponse {
    unimplemented!()
}

#[utoipa::path(
    get,
    path = "/api/webhooks",
    tag = "Webhooks",
    responses(
        (status = 200, description = "List of webhooks", body = WebhookListResponse)
    ),
    security(
        ("api_key" = [])
    )
)]
#[allow(dead_code)]
async fn list_webhooks() -> WebhookListResponse {
    unimplemented!()
}

#[utoipa::path(
    delete,
    path = "/api/webhooks/{id}",
    tag = "Webhooks",
    params(
        ("id" = String, Path, description = "Webhook ID")
    ),
    responses(
        (status = 204, description = "Webhook deleted"),
        (status = 404, description = "Webhook not found")
    ),
    security(
        ("api_key" = [])
    )
)]
#[allow(dead_code)]
async fn delete_webhook() {
    unimplemented!()
}

#[utoipa::path(
    post,
    path = "/api/webhooks/{id}/test",
    tag = "Webhooks",
    params(
        ("id" = String, Path, description = "Webhook ID")
    ),
    responses(
        (status = 200, description = "Test event sent"),
        (status = 404, description = "Webhook not found")
    ),
    security(
        ("api_key" = [])
    )
)]
#[allow(dead_code)]
async fn test_webhook() {
    unimplemented!()
}

#[utoipa::path(
    post,
    path = "/your-webhook-endpoint",
    tag = "Events",
    request_body = WebhookEvent,
    responses(
        (status = 200, description = "Event processed successfully"),
        (status = 401, description = "Invalid signature"),
        (status = 500, description = "Processing error")
    ),
    security(
        ("webhook_signature" = [])
    )
)]
#[allow(dead_code)]
async fn webhook_receiver_example() {
    unimplemented!()
}

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "api_key",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-API-Key"))),
            );
            components.add_security_scheme(
                "webhook_signature",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-Webhook-Signature"))),
            );
        }
    }
}

pub fn generate_openapi_json() -> String {
    ApiDoc::openapi().to_pretty_json().unwrap()
}

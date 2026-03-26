use std::collections::HashMap;

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::common_derives;

type HmacSha256 = Hmac<Sha256>;

#[derive(
    Debug,
    Eq,
    PartialEq,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    specta::Type,
    schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum WebhookType {
    Auth,
    Sync,
    Forward,
    #[serde(other)]
    Unknown,
}

#[derive(
    Debug,
    Eq,
    PartialEq,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    specta::Type,
    schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum AuthOperation {
    Creation,
    Override,
    Refresh,
    Deletion,
    #[serde(other)]
    Unknown,
}

pub fn verify_webhook_signature(secret_key: &str, body: &[u8], signature: &str) -> bool {
    let Ok(mut mac) = HmacSha256::new_from_slice(secret_key.as_bytes()) else {
        return false;
    };
    mac.update(body);
    let expected = hex::encode(mac.finalize().into_bytes());
    expected == signature
}

common_derives! {
    pub struct NangoAuthWebhook {
        pub r#type: WebhookType,
        pub operation: AuthOperation,
        #[serde(rename = "connectionId")]
        pub connection_id: String,
        #[serde(rename = "authMode")]
        pub auth_mode: String,
        #[serde(rename = "providerConfigKey")]
        pub provider_config_key: String,
        pub provider: String,
        pub environment: String,
        pub success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub tags: Option<HashMap<String, String>>,
        #[serde(rename = "endUser")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub end_user: Option<NangoWebhookEndUser>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<NangoWebhookError>,
    }
}

impl NangoAuthWebhook {
    pub fn end_user_id(&self) -> Option<&str> {
        self.end_user
            .as_ref()
            .map(|end_user| end_user.end_user_id.as_str())
            .or_else(|| {
                self.tags
                    .as_ref()
                    .and_then(|tags| tags.get("end_user_id").map(String::as_str))
            })
    }
}

common_derives! {
    pub struct NangoWebhookEndUser {
        #[serde(rename = "endUserId")]
        pub end_user_id: String,
        #[serde(rename = "endUserEmail")]
        pub end_user_email: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub tags: Option<HashMap<String, String>>,
    }
}

common_derives! {
    pub struct NangoWebhookError {
        pub r#type: String,
        pub description: String,
    }
}

common_derives! {
    pub struct NangoSyncWebhook {
        pub r#type: WebhookType,
        #[serde(rename = "connectionId")]
        pub connection_id: String,
        #[serde(rename = "providerConfigKey")]
        pub provider_config_key: String,
        #[serde(rename = "syncName")]
        pub sync_name: String,
        pub model: String,
        #[serde(rename = "syncType")]
        pub sync_type: String,
        pub success: bool,
        #[serde(rename = "modifiedAfter")]
        pub modified_after: String,
        #[serde(rename = "responseResults")]
        pub response_results: Option<NangoSyncWebhookResults>,
    }
}

common_derives! {
    pub struct NangoSyncWebhookResults {
        pub added: u64,
        pub updated: u64,
        pub deleted: u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_webhook_signature_valid() {
        let secret = "test-secret-key";
        let body = b"test-body";

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let signature = hex::encode(mac.finalize().into_bytes());

        assert!(verify_webhook_signature(secret, body, &signature));
    }

    #[test]
    fn test_verify_webhook_signature_invalid() {
        let secret = "test-secret-key";
        let body = b"test-body";
        assert!(!verify_webhook_signature(secret, body, "invalid-signature"));
    }

    #[test]
    fn test_deserialize_auth_webhook() {
        let json = r#"{
            "type": "auth",
            "operation": "creation",
            "connectionId": "conn-123",
            "authMode": "OAUTH2",
            "providerConfigKey": "google-calendar",
            "provider": "google-calendar",
            "environment": "DEV",
            "success": true,
            "endUser": {
                "endUserId": "user-456",
                "endUserEmail": "user@example.com",
                "tags": { "organizationId": "org-789" }
            }
        }"#;

        let webhook: NangoAuthWebhook = serde_json::from_str(json).unwrap();
        assert_eq!(webhook.r#type, WebhookType::Auth);
        assert_eq!(webhook.operation, AuthOperation::Creation);
        assert_eq!(webhook.connection_id, "conn-123");
        assert_eq!(webhook.auth_mode, "OAUTH2");
        assert_eq!(webhook.provider_config_key, "google-calendar");
        assert!(webhook.success);
        let end_user = webhook.end_user.as_ref().unwrap();
        assert_eq!(end_user.end_user_id, "user-456");
        assert_eq!(end_user.end_user_email.as_deref(), Some("user@example.com"));
        assert_eq!(
            end_user
                .tags
                .as_ref()
                .and_then(|t| t.get("organizationId"))
                .map(|s| s.as_str()),
            Some("org-789")
        );
        assert!(webhook.tags.is_none());
        assert!(webhook.error.is_none());
    }

    #[test]
    fn test_deserialize_auth_webhook_with_error() {
        let json = r#"{
            "type": "auth",
            "operation": "refresh",
            "connectionId": "conn-123",
            "authMode": "OAUTH2",
            "providerConfigKey": "google-calendar",
            "provider": "google-calendar",
            "environment": "DEV",
            "success": false,
            "endUser": {
                "endUserId": "user-456"
            },
            "error": {
                "type": "refresh_token_error",
                "description": "Token expired"
            }
        }"#;

        let webhook: NangoAuthWebhook = serde_json::from_str(json).unwrap();
        assert!(!webhook.success);
        assert_eq!(webhook.operation, AuthOperation::Refresh);
        assert_eq!(webhook.end_user_id(), Some("user-456"));
        assert!(webhook.error.is_some());
        let error = webhook.error.unwrap();
        assert_eq!(error.r#type, "refresh_token_error");
        assert_eq!(error.description, "Token expired");
    }

    #[test]
    fn test_deserialize_auth_webhook_with_top_level_tags_only() {
        let json = r#"{
            "type": "auth",
            "operation": "creation",
            "connectionId": "conn-123",
            "authMode": "OAUTH2",
            "providerConfigKey": "google-calendar",
            "provider": "google-calendar",
            "environment": "DEV",
            "success": true,
            "tags": {
                "end_user_id": "user-456",
                "organization_id": "org-789"
            }
        }"#;

        let webhook: NangoAuthWebhook = serde_json::from_str(json).unwrap();
        assert_eq!(webhook.end_user_id(), Some("user-456"));
        assert!(webhook.end_user.is_none());
        assert_eq!(
            webhook
                .tags
                .as_ref()
                .and_then(|t| t.get("organization_id"))
                .map(|s| s.as_str()),
            Some("org-789")
        );
    }

    #[test]
    fn test_deserialize_sync_webhook() {
        let json = r#"{
            "type": "sync",
            "connectionId": "conn-123",
            "providerConfigKey": "google-calendar",
            "syncName": "calendar-events",
            "model": "CalendarEvent",
            "syncType": "INCREMENTAL",
            "success": true,
            "modifiedAfter": "2025-05-21T18:52:49.838Z",
            "responseResults": {
                "added": 5,
                "updated": 2,
                "deleted": 1
            }
        }"#;

        let webhook: NangoSyncWebhook = serde_json::from_str(json).unwrap();
        assert_eq!(webhook.r#type, WebhookType::Sync);
        assert_eq!(webhook.connection_id, "conn-123");
        assert_eq!(webhook.sync_name, "calendar-events");
        assert_eq!(webhook.sync_type, "INCREMENTAL");
        assert!(webhook.success);
        let results = webhook.response_results.unwrap();
        assert_eq!(results.added, 5);
        assert_eq!(results.updated, 2);
        assert_eq!(results.deleted, 1);
    }
}

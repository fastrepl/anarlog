use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

#[cfg(feature = "client")]
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct Session {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub token_type: String,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub expires_at: Option<u64>,
    pub user: SessionUser,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Session {
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token
            .as_deref()
            .filter(|token| !token.is_empty())
    }

    pub fn expires_soon(&self, now: SystemTime, skew: Duration) -> bool {
        let Some(expires_at) = self.expires_at else {
            return true;
        };
        let now_secs = now
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        now_secs.saturating_add(skew.as_secs()) >= expires_at
    }

    pub fn requires_refresh(&self, now: SystemTime, skew: Duration) -> bool {
        self.expires_soon(now, skew)
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct SessionUser {
    pub id: String,
    #[serde(default)]
    pub aud: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email_confirmed_at: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<String>,
    #[serde(default)]
    pub recovery_sent_at: Option<String>,
    #[serde(default)]
    pub last_sign_in_at: Option<String>,
    #[serde(default)]
    pub app_metadata: Option<AppMetadata>,
    pub user_metadata: Option<UserMetadata>,
    #[serde(default)]
    pub identities: Vec<Identity>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub is_anonymous: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct AppMetadata {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub providers: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct UserMetadata {
    #[serde(default)]
    pub email: Option<String>,
    pub full_name: Option<String>,
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default)]
    pub email_verified: Option<bool>,
    #[serde(default)]
    pub phone_verified: Option<bool>,
    pub stripe_customer_id: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct Identity {
    #[serde(default)]
    pub identity_id: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[cfg(feature = "client")]
pub fn find_session(data: &HashMap<String, String>) -> crate::client::Result<Option<Session>> {
    let Some(session_str) = data
        .iter()
        .find_map(|(k, v)| k.ends_with("-auth-token").then_some(v.as_str()))
    else {
        return Ok(None);
    };
    Ok(Some(serde_json::from_str(session_str)?))
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "client")]
    use std::collections::HashMap;

    use super::*;

    #[cfg(feature = "client")]
    fn make_data(key: &str, session_json: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert(key.to_string(), session_json.to_string());
        m
    }

    #[cfg(feature = "client")]
    const SESSION_JSON: &str = r#"{
        "access_token": "eyJhbGciOiJFUzI1NiJ9.test",
        "token_type": "bearer",
        "expires_in": 3600,
        "expires_at": 9999999999,
        "refresh_token": "refresh123",
        "user": {
            "id": "818fe58f-afe9-42da-b288-f7d14213b6b4",
            "email": "user@example.com",
            "user_metadata": {
                "full_name": "Test User",
                "avatar_url": "https://example.com/avatar.png",
                "stripe_customer_id": "cus_test123"
            }
        }
    }"#;

    #[cfg(feature = "client")]
    #[test]
    fn returns_none_for_empty_store() {
        let data = HashMap::new();
        assert!(find_session(&data).unwrap().is_none());
    }

    #[cfg(feature = "client")]
    #[test]
    fn returns_none_when_no_auth_token_key() {
        let data = make_data("sb-auth-something-else", SESSION_JSON);
        assert!(find_session(&data).unwrap().is_none());
    }

    #[cfg(feature = "client")]
    #[test]
    fn parses_access_token() {
        let data = make_data("sb-auth-auth-token", SESSION_JSON);
        let session = find_session(&data).unwrap().unwrap();
        assert_eq!(session.access_token, "eyJhbGciOiJFUzI1NiJ9.test");
        assert_eq!(session.refresh_token.as_deref(), Some("refresh123"));
        assert_eq!(session.token_type, "bearer");
        assert_eq!(session.expires_at, Some(9999999999));
    }

    #[cfg(feature = "client")]
    #[test]
    fn parses_user_fields() {
        let data = make_data("sb-auth-auth-token", SESSION_JSON);
        let session = find_session(&data).unwrap().unwrap();
        assert_eq!(session.user.id, "818fe58f-afe9-42da-b288-f7d14213b6b4");
        assert_eq!(session.user.email.as_deref(), Some("user@example.com"));
        assert_eq!(session.user.aud.as_deref(), None);
    }

    #[cfg(feature = "client")]
    #[test]
    fn parses_user_metadata() {
        let data = make_data("sb-auth-auth-token", SESSION_JSON);
        let session = find_session(&data).unwrap().unwrap();
        let meta = session.user.user_metadata.unwrap();
        assert_eq!(meta.full_name.as_deref(), Some("Test User"));
        assert_eq!(meta.stripe_customer_id.as_deref(), Some("cus_test123"));
    }

    #[cfg(feature = "client")]
    #[test]
    fn tolerates_missing_user_metadata() {
        let json = r#"{
            "access_token": "tok",
            "token_type": "bearer",
            "expires_in": 3600,
            "expires_at": 9999999999,
            "refresh_token": "r",
            "user": { "id": "uid-1", "email": null }
        }"#;
        let data = make_data("sb-projectref-auth-token", json);
        let session = find_session(&data).unwrap().unwrap();
        assert!(session.user.user_metadata.is_none());
    }

    #[cfg(feature = "client")]
    #[test]
    fn returns_err_for_invalid_json() {
        let data = make_data("sb-auth-auth-token", "not-json");
        assert!(find_session(&data).is_err());
    }

    #[test]
    fn expires_soon_when_expiry_missing() {
        let session: Session = serde_json::from_str(
            r#"{
                "access_token": "tok",
                "refresh_token": "r",
                "user": { "id": "uid-1", "email": null }
            }"#,
        )
        .unwrap();
        assert!(session.requires_refresh(UNIX_EPOCH, Duration::from_secs(60)));
    }

    #[cfg(feature = "client")]
    #[test]
    fn requires_refresh_respects_skew() {
        let data = make_data("sb-auth-auth-token", SESSION_JSON);
        let session = find_session(&data).unwrap().unwrap();

        let before_expiry = UNIX_EPOCH + Duration::from_secs(9999999900);
        assert!(!session.expires_soon(before_expiry, Duration::from_secs(10)));
        assert!(session.expires_soon(before_expiry, Duration::from_secs(200)));
    }

    #[cfg(feature = "client")]
    #[test]
    fn preserves_unknown_fields_when_roundtripping() {
        let json = r#"{
            "access_token": "tok",
            "token_type": "bearer",
            "expires_in": 3600,
            "expires_at": 9999999999,
            "refresh_token": "r",
            "provider_token": "provider-tok",
            "user": {
                "id": "uid-1",
                "email": "user@example.com",
                "aud": "authenticated",
                "app_metadata": {
                    "provider": "google",
                    "providers": ["google", "github"]
                },
                "user_metadata": {
                    "full_name": "Test User",
                    "preferred_username": "tester"
                },
                "identities": [
                    {
                        "identity_id": "ident-1",
                        "provider": "google",
                        "custom_field": true
                    }
                ],
                "custom_user_field": "keep-me"
            }
        }"#;

        let data = make_data("sb-auth-auth-token", json);
        let session = find_session(&data).unwrap().unwrap();
        let serialized = serde_json::to_value(&session).unwrap();

        assert_eq!(serialized["provider_token"], "provider-tok");
        assert_eq!(serialized["user"]["custom_user_field"], "keep-me");
        assert_eq!(
            serialized["user"]["user_metadata"]["preferred_username"],
            "tester"
        );
        assert_eq!(serialized["user"]["identities"][0]["custom_field"], true);
    }
}

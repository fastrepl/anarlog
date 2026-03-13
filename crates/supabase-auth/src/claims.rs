use chrono::{DateTime, Utc};

use crate::error::Error;

// https://docs.stripe.com/api/subscriptions/object#subscription_object-status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Incomplete,
    IncompleteExpired,
    Trialing,
    Active,
    PastDue,
    Canceled,
    Unpaid,
    Paused,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Claims {
    pub sub: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub entitlements: Vec<String>,
    #[serde(default)]
    pub subscription_status: Option<SubscriptionStatus>,
    #[serde(default, with = "chrono::serde::ts_seconds_option")]
    #[specta(type = Option<i64>)]
    pub trial_end: Option<DateTime<Utc>>,
}

impl Claims {
    pub fn is_pro(&self) -> bool {
        self.entitlements.contains(&"hyprnote_pro".to_string())
    }

    pub fn decode_insecure(token: &str) -> Result<Self, Error> {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(Error::InvalidToken);
        }

        let payload = URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|_| Error::InvalidToken)?;

        serde_json::from_slice(&payload).map_err(|_| Error::InvalidToken)
    }
}

#[cfg(test)]
mod tests {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use chrono::Datelike;

    use super::*;

    fn make_test_token(payload: &str) -> String {
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(payload);
        format!("{}.{}.fake_signature", header, payload)
    }

    #[test]
    fn test_decode_claims() {
        let payload = r#"{
            "sub": "user-123",
            "email": "test@example.com",
            "entitlements": ["hyprnote_pro"],
            "subscription_status": "trialing",
            "trial_end": 1771406553
        }"#;
        let token = make_test_token(payload);

        let claims = Claims::decode_insecure(&token).unwrap();
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.email, Some("test@example.com".to_string()));
        assert_eq!(claims.entitlements, vec!["hyprnote_pro"]);
        assert!(matches!(
            claims.subscription_status,
            Some(SubscriptionStatus::Trialing)
        ));
        assert_eq!(claims.trial_end.unwrap().year(), 2026);
    }

    #[test]
    fn test_decode_claims_minimal() {
        let payload = r#"{"sub": "user-456"}"#;
        let token = make_test_token(payload);

        let claims = Claims::decode_insecure(&token).unwrap();
        assert_eq!(claims.sub, "user-456");
        assert_eq!(claims.email, None);
        assert!(claims.entitlements.is_empty());
        assert!(claims.subscription_status.is_none());
        assert!(claims.trial_end.is_none());
    }

    #[test]
    fn test_decode_invalid_token() {
        assert!(Claims::decode_insecure("invalid").is_err());
        assert!(Claims::decode_insecure("a.b").is_err());
        assert!(Claims::decode_insecure("a.!!!.c").is_err());
    }
}

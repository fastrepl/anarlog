use chrono::{DateTime, Utc};

pub use error::{Error, Result};

mod error;

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
    #[serde(default)]
    pub has_payment_method: Option<bool>,
}

impl Claims {
    pub fn has_active_trial(&self) -> bool {
        self.has_active_trial_at(Utc::now())
    }

    fn has_active_trial_at(&self, now: DateTime<Utc>) -> bool {
        matches!(self.subscription_status, Some(SubscriptionStatus::Trialing))
            && self.trial_end.as_ref().is_some_and(|end| end > &now)
    }

    pub fn has_entitlement(&self, entitlement: &str) -> bool {
        if entitlement == "hyprnote_pro"
            && matches!(self.subscription_status, Some(SubscriptionStatus::Trialing))
        {
            return self.has_active_trial();
        }

        self.entitlements.iter().any(|value| value == entitlement)
    }

    pub fn is_pro(&self) -> bool {
        self.has_entitlement("hyprnote_pro")
    }

    pub fn is_lite(&self) -> bool {
        self.has_entitlement("hyprnote_lite")
    }

    pub fn is_paid(&self) -> bool {
        self.is_pro() || self.is_lite()
    }

    pub fn decode_insecure(token: &str) -> Result<Self> {
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
    use chrono::{Datelike, Duration};

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
            "trial_end": 1771406553,
            "has_payment_method": true
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
        assert_eq!(claims.has_payment_method, Some(true));
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
        assert!(claims.has_payment_method.is_none());
    }

    #[test]
    fn test_decode_claims_lite() {
        let payload = r#"{
            "sub": "user-789",
            "entitlements": ["hyprnote_lite"],
            "subscription_status": "active"
        }"#;
        let token = make_test_token(payload);

        let claims = Claims::decode_insecure(&token).unwrap();
        assert!(!claims.is_pro());
        assert!(claims.is_lite());
        assert!(claims.is_paid());
    }

    #[test]
    fn test_is_paid_with_pro() {
        let payload = r#"{
            "sub": "user-100",
            "entitlements": ["hyprnote_pro"]
        }"#;
        let token = make_test_token(payload);

        let claims = Claims::decode_insecure(&token).unwrap();
        assert!(claims.is_pro());
        assert!(!claims.is_lite());
        assert!(claims.is_paid());
    }

    #[test]
    fn test_is_paid_with_no_entitlements() {
        let payload = r#"{"sub": "user-200"}"#;
        let token = make_test_token(payload);

        let claims = Claims::decode_insecure(&token).unwrap();
        assert!(!claims.is_pro());
        assert!(!claims.is_lite());
        assert!(!claims.is_paid());
    }

    #[test]
    fn test_active_status_without_entitlement_is_not_paid() {
        let payload = r#"{
            "sub": "user-201",
            "subscription_status": "active"
        }"#;
        let token = make_test_token(payload);

        let claims = Claims::decode_insecure(&token).unwrap();
        assert!(!claims.is_pro());
        assert!(!claims.is_lite());
        assert!(!claims.is_paid());
    }

    #[test]
    fn test_active_trial_grants_pro_access() {
        let claims = Claims {
            sub: "trial-user".to_string(),
            email: None,
            entitlements: vec![],
            subscription_status: Some(SubscriptionStatus::Trialing),
            trial_end: Some(Utc::now() + Duration::minutes(5)),
            has_payment_method: Some(false),
        };

        assert!(claims.has_active_trial());
        assert!(claims.has_entitlement("hyprnote_pro"));
        assert!(claims.is_pro());
        assert!(claims.is_paid());
    }

    #[test]
    fn test_expired_or_unbounded_trial_fails_closed() {
        let mut claims = Claims {
            sub: "trial-user".to_string(),
            email: None,
            entitlements: vec!["hyprnote_pro".to_string()],
            subscription_status: Some(SubscriptionStatus::Trialing),
            trial_end: Some(Utc::now() - Duration::minutes(5)),
            has_payment_method: Some(false),
        };

        assert!(!claims.has_active_trial());
        assert!(!claims.is_pro());

        claims.trial_end = None;

        assert!(!claims.has_active_trial());
        assert!(!claims.is_pro());
    }

    #[test]
    fn test_decode_invalid_token() {
        assert!(Claims::decode_insecure("invalid").is_err());
        assert!(Claims::decode_insecure("a.b").is_err());
        assert!(Claims::decode_insecure("a.!!!.c").is_err());
    }
}

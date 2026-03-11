use std::collections::HashMap;

#[derive(serde::Deserialize)]
pub struct Session {
    pub access_token: String,
    pub user: SessionUser,
}

#[derive(serde::Deserialize)]
pub struct SessionUser {
    pub id: String,
    pub email: Option<String>,
    pub user_metadata: Option<UserMetadata>,
}

#[derive(serde::Deserialize)]
pub struct UserMetadata {
    pub full_name: Option<String>,
    pub avatar_url: Option<String>,
    pub stripe_customer_id: Option<String>,
}

pub fn find_session(data: &HashMap<String, String>) -> Result<Option<Session>, super::Error> {
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
    use super::*;

    fn make_data(key: &str, session_json: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert(key.to_string(), session_json.to_string());
        m
    }

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

    #[test]
    fn returns_none_for_empty_store() {
        let data = HashMap::new();
        assert!(find_session(&data).unwrap().is_none());
    }

    #[test]
    fn returns_none_when_no_auth_token_key() {
        let data = make_data("sb-auth-something-else", SESSION_JSON);
        assert!(find_session(&data).unwrap().is_none());
    }

    #[test]
    fn parses_access_token() {
        let data = make_data("sb-auth-auth-token", SESSION_JSON);
        let session = find_session(&data).unwrap().unwrap();
        assert_eq!(session.access_token, "eyJhbGciOiJFUzI1NiJ9.test");
    }

    #[test]
    fn parses_user_fields() {
        let data = make_data("sb-auth-auth-token", SESSION_JSON);
        let session = find_session(&data).unwrap().unwrap();
        assert_eq!(session.user.id, "818fe58f-afe9-42da-b288-f7d14213b6b4");
        assert_eq!(session.user.email.as_deref(), Some("user@example.com"));
    }

    #[test]
    fn parses_user_metadata() {
        let data = make_data("sb-auth-auth-token", SESSION_JSON);
        let session = find_session(&data).unwrap().unwrap();
        let meta = session.user.user_metadata.unwrap();
        assert_eq!(meta.full_name.as_deref(), Some("Test User"));
        assert_eq!(meta.stripe_customer_id.as_deref(), Some("cus_test123"));
    }

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

    #[test]
    fn returns_err_for_invalid_json() {
        let data = make_data("sb-auth-auth-token", "not-json");
        assert!(find_session(&data).is_err());
    }
}

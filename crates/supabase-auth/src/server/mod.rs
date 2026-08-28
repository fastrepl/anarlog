use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::Deserialize;

mod error;
mod jwks;
pub use error::{Error, Result};

use jwks::*;

#[derive(Clone)]
pub struct SupabaseAuth {
    jwks: CachedJwks,
    issuer: String,
}

#[derive(Deserialize)]
struct OAuthClaims {
    #[serde(flatten)]
    claims: crate::Claims,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    scope: String,
}

impl SupabaseAuth {
    pub fn new(supabase_url: &str) -> Self {
        let supabase_url = supabase_url.trim_end_matches('/');
        let jwks_url = format!("{}/auth/v1/.well-known/jwks.json", supabase_url);
        Self {
            jwks: CachedJwks::new(jwks_url),
            issuer: format!("{supabase_url}/auth/v1"),
        }
    }

    pub fn extract_token(auth_header: &str) -> Option<&str> {
        auth_header
            .strip_prefix("Bearer ")
            .or_else(|| auth_header.strip_prefix("bearer "))
            .or_else(|| auth_header.strip_prefix("Token "))
            .or_else(|| auth_header.strip_prefix("token "))
    }

    pub async fn verify_token(&self, token: &str) -> Result<crate::Claims> {
        let (algorithm, decoding_key) = self.decoding_key(token).await?;

        let mut validation = Validation::new(algorithm);
        validation.validate_exp = true;
        validation.set_audience(&["authenticated"]);

        let token_data = jsonwebtoken::decode::<OAuthClaims>(token, &decoding_key, &validation)
            .map_err(|_| Error::InvalidToken)?;
        // Connector tokens must not authenticate app sessions, even if an older
        // hook also minted the `authenticated` audience.
        if !token_data.claims.client_id.trim().is_empty() {
            return Err(Error::InvalidToken);
        }

        Ok(token_data.claims.claims)
    }

    pub async fn verify_oauth_token(
        &self,
        token: &str,
        resource: &str,
        required_scopes: &[&str],
    ) -> Result<crate::Claims> {
        let (algorithm, decoding_key) = self.decoding_key(token).await?;

        let mut validation = Validation::new(algorithm);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.set_audience(&[resource]);
        validation.set_issuer(&[&self.issuer]);

        let token_data = jsonwebtoken::decode::<OAuthClaims>(token, &decoding_key, &validation)
            .map_err(|_| Error::InvalidToken)?;
        if token_data.claims.client_id.trim().is_empty() {
            return Err(Error::MissingOAuthClient);
        }
        for required_scope in required_scopes {
            if !token_data
                .claims
                .scope
                .split_ascii_whitespace()
                .any(|scope| scope == *required_scope)
            {
                return Err(Error::MissingScope((*required_scope).to_string()));
            }
        }

        Ok(token_data.claims.claims)
    }

    async fn decoding_key(&self, token: &str) -> Result<(Algorithm, DecodingKey)> {
        let header = jsonwebtoken::decode_header(token).map_err(|_| Error::InvalidToken)?;

        let jwks = self.jwks.get().await?;

        let kid = header.kid.as_deref().ok_or(Error::InvalidToken)?;
        let jwk = jwks.find(kid).ok_or(Error::InvalidToken)?;

        let algorithm = match jwk.common.key_algorithm {
            Some(jsonwebtoken::jwk::KeyAlgorithm::RS256) => Algorithm::RS256,
            Some(jsonwebtoken::jwk::KeyAlgorithm::ES256) => Algorithm::ES256,
            _ => return Err(Error::InvalidToken),
        };

        DecodingKey::from_jwk(jwk)
            .map(|key| (algorithm, key))
            .map_err(|_| Error::InvalidToken)
    }

    pub async fn require_entitlement(
        &self,
        token: &str,
        entitlement: &str,
    ) -> Result<crate::Claims> {
        let claims = self.verify_token(token).await?;

        if !claims.has_entitlement(entitlement) {
            return Err(Error::MissingEntitlement(entitlement.to_string()));
        }

        Ok(claims)
    }

    pub async fn require_any_entitlement(
        &self,
        token: &str,
        entitlements: &[&str],
    ) -> Result<crate::Claims> {
        let claims = self.verify_token(token).await?;

        let has_any = entitlements
            .iter()
            .any(|entitlement| claims.has_entitlement(entitlement));

        if !has_any {
            return Err(Error::MissingEntitlement(entitlements.join(" or ")));
        }

        Ok(claims)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use super::{Error, SupabaseAuth};

    const TEST_KEY_ID: &str = "oauth-test";
    const TEST_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----\n\
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgWTFfCGljY6aw3Hrt\n\
kHmPRiazukxPLb6ilpRAewjW8nihRANCAATDskChT+Altkm9X7MI69T3IUmrQU0L\n\
950IxEzvw/x5BMEINRMrXLBJhqzO9Bm+d6JbqA21YQmd1Kt4RzLJR1W+\n\
-----END PRIVATE KEY-----";

    async fn oauth_auth() -> (MockServer, SupabaseAuth) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/auth/v1/.well-known/jwks.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "keys": [{
                    "kty": "EC",
                    "use": "sig",
                    "crv": "P-256",
                    "kid": TEST_KEY_ID,
                    "x": "w7JAoU_gJbZJvV-zCOvU9yFJq0FNC_edCMRM78P8eQQ",
                    "y": "wQg1EytcsEmGrM70Gb53oluoDbVhCZ3Uq3hHMslHVb4",
                    "alg": "ES256"
                }]
            })))
            .mount(&server)
            .await;
        let auth = SupabaseAuth::new(&server.uri());
        (server, auth)
    }

    fn signed_token(claims: serde_json::Value) -> String {
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(TEST_KEY_ID.to_string());
        encode(
            &header,
            &claims,
            &EncodingKey::from_ec_pem(TEST_PRIVATE_KEY.as_bytes()).unwrap(),
        )
        .unwrap()
    }

    fn expires_at() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3_600
    }

    fn oauth_token(issuer: &str, audience: &str, scope: &str, client_id: &str) -> String {
        signed_token(json!({
            "sub": "oauth-user",
            "iss": issuer,
            "aud": [audience],
            "exp": expires_at(),
            "client_id": client_id,
            "scope": scope,
        }))
    }

    #[test]
    fn extract_token_accepts_bearer_prefix() {
        assert_eq!(
            SupabaseAuth::extract_token("Bearer test-token"),
            Some("test-token")
        );
        assert_eq!(
            SupabaseAuth::extract_token("bearer test-token"),
            Some("test-token")
        );
    }

    #[test]
    fn extract_token_accepts_token_prefix_for_backward_compat() {
        assert_eq!(
            SupabaseAuth::extract_token("Token test-token"),
            Some("test-token")
        );
        assert_eq!(
            SupabaseAuth::extract_token("token test-token"),
            Some("test-token")
        );
    }

    #[test]
    fn extract_token_rejects_unknown_prefix() {
        assert_eq!(SupabaseAuth::extract_token("Basic test-token"), None);
    }

    #[tokio::test]
    async fn oauth_token_requires_issuer_audience_client_and_scopes() {
        let (server, auth) = oauth_auth().await;
        let resource = "https://api.anarlog.so/mcp";
        let token = oauth_token(
            &format!("{}/auth/v1", server.uri()),
            resource,
            "openid email",
            "chatgpt-client",
        );

        let claims = auth
            .verify_oauth_token(&token, resource, &["openid", "email"])
            .await
            .unwrap();

        assert_eq!(claims.sub, "oauth-user");
        assert!(matches!(
            auth.verify_oauth_token(&token, resource, &["profile"]).await,
            Err(Error::MissingScope(scope)) if scope == "profile"
        ));
        let wrong_resource = auth
            .verify_oauth_token(&token, "https://api.anarlog.so/other", &["openid"])
            .await;
        assert!(matches!(wrong_resource, Err(Error::InvalidToken)));
    }

    #[tokio::test]
    async fn oauth_tokens_are_rejected_as_session_tokens() {
        let (server, auth) = oauth_auth().await;
        let resource = "https://api.anarlog.so/mcp";
        let issuer = format!("{}/auth/v1", server.uri());
        let mcp_token = oauth_token(&issuer, resource, "openid email", "chatgpt-client");
        let dual_audience_token = signed_token(json!({
            "sub": "oauth-user",
            "iss": issuer,
            "aud": ["authenticated", resource],
            "exp": expires_at(),
            "client_id": "chatgpt-client",
            "scope": "openid email",
        }));
        let session_token = signed_token(json!({
            "sub": "session-user",
            "iss": issuer,
            "aud": "authenticated",
            "exp": expires_at(),
        }));

        assert!(matches!(
            auth.verify_token(&mcp_token).await,
            Err(Error::InvalidToken)
        ));
        assert!(matches!(
            auth.verify_token(&dual_audience_token).await,
            Err(Error::InvalidToken)
        ));
        assert_eq!(
            auth.verify_token(&session_token).await.unwrap().sub,
            "session-user"
        );
    }
}

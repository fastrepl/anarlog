use jsonwebtoken::{Algorithm, DecodingKey, Validation};

mod jwks;
use jwks::*;

#[derive(Clone)]
pub struct SupabaseAuth {
    jwks: CachedJwks,
}

impl SupabaseAuth {
    pub fn new(supabase_url: &str) -> Self {
        let jwks_url = format!(
            "{}/auth/v1/.well-known/jwks.json",
            supabase_url.trim_end_matches('/')
        );
        Self {
            jwks: CachedJwks::new(jwks_url),
        }
    }

    pub fn extract_token(auth_header: &str) -> Option<&str> {
        auth_header
            .strip_prefix("Bearer ")
            .or_else(|| auth_header.strip_prefix("bearer "))
            .or_else(|| auth_header.strip_prefix("Token "))
            .or_else(|| auth_header.strip_prefix("token "))
    }

    pub async fn verify_token(&self, token: &str) -> Result<crate::Claims, crate::Error> {
        let header = jsonwebtoken::decode_header(token).map_err(|_| crate::Error::InvalidToken)?;

        let jwks = self.jwks.get().await?;

        let kid = header.kid.as_deref().ok_or(crate::Error::InvalidToken)?;
        let jwk = jwks.find(kid).ok_or(crate::Error::InvalidToken)?;

        let algorithm = match jwk.common.key_algorithm {
            Some(jsonwebtoken::jwk::KeyAlgorithm::RS256) => Algorithm::RS256,
            Some(jsonwebtoken::jwk::KeyAlgorithm::ES256) => Algorithm::ES256,
            _ => return Err(crate::Error::InvalidToken),
        };

        let decoding_key = DecodingKey::from_jwk(jwk).map_err(|_| crate::Error::InvalidToken)?;

        let mut validation = Validation::new(algorithm);
        validation.validate_exp = true;
        validation.set_audience(&["authenticated"]);

        let token_data = jsonwebtoken::decode::<crate::Claims>(token, &decoding_key, &validation)
            .map_err(|_| crate::Error::InvalidToken)?;

        Ok(token_data.claims)
    }

    pub async fn require_entitlement(
        &self,
        token: &str,
        entitlement: &str,
    ) -> Result<crate::Claims, crate::Error> {
        let claims = self.verify_token(token).await?;

        if !claims.entitlements.contains(&entitlement.to_string()) {
            return Err(crate::Error::MissingEntitlement(entitlement.to_string()));
        }

        Ok(claims)
    }

    pub async fn require_any_entitlement(
        &self,
        token: &str,
        entitlements: &[&str],
    ) -> Result<crate::Claims, crate::Error> {
        let claims = self.verify_token(token).await?;

        let has_any = entitlements
            .iter()
            .any(|e| claims.entitlements.contains(&e.to_string()));

        if !has_any {
            return Err(crate::Error::MissingEntitlement(entitlements.join(" or ")));
        }

        Ok(claims)
    }
}

#[cfg(test)]
mod tests {
    use super::SupabaseAuth;

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
}

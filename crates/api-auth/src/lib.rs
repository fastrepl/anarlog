use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use hypr_supabase_auth::{Error as SupabaseAuthError, server::SupabaseAuth};

pub use hypr_supabase_auth::Claims;

#[derive(Clone)]
pub struct AuthContext {
    pub token: String,
    pub claims: Claims,
}

#[derive(Clone)]
pub struct AuthState {
    inner: SupabaseAuth,
    required_entitlement: Option<String>,
}

impl AuthState {
    pub fn new(supabase_url: &str) -> Self {
        Self {
            inner: SupabaseAuth::new(supabase_url),
            required_entitlement: None,
        }
    }

    pub fn with_required_entitlement(mut self, entitlement: impl Into<String>) -> Self {
        self.required_entitlement = Some(entitlement.into());
        self
    }

    pub fn extract_token(auth_header: &str) -> Option<&str> {
        SupabaseAuth::extract_token(auth_header)
    }

    pub async fn verify_token(&self, token: &str) -> Result<Claims, AuthError> {
        self.inner.verify_token(token).await.map_err(AuthError)
    }
}

pub struct AuthError(SupabaseAuthError);

impl From<SupabaseAuthError> for AuthError {
    fn from(err: SupabaseAuthError) -> Self {
        Self(err)
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self.0 {
            SupabaseAuthError::MissingAuthHeader => {
                (StatusCode::UNAUTHORIZED, "missing_authorization_header")
            }
            SupabaseAuthError::InvalidAuthHeader => {
                (StatusCode::UNAUTHORIZED, "invalid_authorization_header")
            }
            SupabaseAuthError::JwksFetchFailed => {
                (StatusCode::INTERNAL_SERVER_ERROR, "jwks_fetch_failed")
            }
            SupabaseAuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "invalid_token"),
            SupabaseAuthError::MissingEntitlement(_) => {
                (StatusCode::FORBIDDEN, "subscription_required")
            }
        };
        (status, message).into_response()
    }
}

pub async fn require_auth(
    State(state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(SupabaseAuthError::MissingAuthHeader)?;

    let token = SupabaseAuth::extract_token(auth_header)
        .ok_or(SupabaseAuthError::InvalidAuthHeader)?
        .to_owned();

    let claims = match &state.required_entitlement {
        Some(entitlement) => state.inner.require_entitlement(&token, entitlement).await?,
        None => state.inner.verify_token(&token).await?,
    };

    request
        .extensions_mut()
        .insert(AuthContext { token, claims });

    Ok(next.run(request).await)
}

pub async fn optional_auth(
    State(state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some(auth_header) = request
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        && let Some(token) = SupabaseAuth::extract_token(auth_header)
    {
        let token = token.to_owned();
        if let Ok(claims) = state.inner.verify_token(&token).await {
            request
                .extensions_mut()
                .insert(AuthContext { token, claims });
        }
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_error_missing_header() {
        let err = AuthError(SupabaseAuthError::MissingAuthHeader);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_error_invalid_header() {
        let err = AuthError(SupabaseAuthError::InvalidAuthHeader);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_error_jwks_fetch_failed() {
        let err = AuthError(SupabaseAuthError::JwksFetchFailed);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_auth_error_invalid_token() {
        let err = AuthError(SupabaseAuthError::InvalidToken);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_error_missing_entitlement() {
        let err = AuthError(SupabaseAuthError::MissingEntitlement("pro".to_string()));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_auth_state_new() {
        let state = AuthState::new("https://example.supabase.co");
        assert_eq!(state.required_entitlement, None);
    }

    #[test]
    fn test_auth_state_with_required_entitlement() {
        let state =
            AuthState::new("https://example.supabase.co").with_required_entitlement("hyprnote_pro");
        assert_eq!(state.required_entitlement, Some("hyprnote_pro".to_string()));
    }
}

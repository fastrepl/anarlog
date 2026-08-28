use anlg_api_auth::{AuthContext, Claims, OAuthTokenError};
use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};

use crate::{CloudApiError, state::AppState};

pub async fn require_cloud_connector_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, CloudApiError> {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(anlg_api_auth::AuthState::extract_token)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .ok_or_else(|| unauthorized(&state, None))?;

    if token.starts_with("anl_") {
        return require_api_key(&state, &token, request, next).await;
    }

    let claims = state
        .verify_oauth_token(&token)
        .await
        .map_err(|error| match error {
            OAuthTokenError::Invalid => unauthorized(
                &state,
                Some(("invalid_token", "Access token is invalid or expired")),
            ),
            OAuthTokenError::InsufficientScope => {
                CloudApiError::InsufficientScope(state.oauth().challenge(Some((
                    "insufficient_scope",
                    "Access token is missing a required scope",
                ))))
            }
        })?;
    enforce_user_status(&state, &claims.sub).await?;
    request
        .extensions_mut()
        .insert(AuthContext { token, claims });

    Ok(next.run(request).await)
}

async fn require_api_key(
    state: &AppState,
    token: &str,
    mut request: Request,
    next: Next,
) -> Result<Response, CloudApiError> {
    if token.len() != 68 {
        return Err(unauthorized(state, None));
    }
    let key_hash = Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let verified = state
        .verify_key(&key_hash)
        .await
        .map_err(|error| CloudApiError::Internal(error.to_string()))?
        .ok_or_else(|| unauthorized(state, None))?;

    match verified.status.as_str() {
        "ok" => {}
        "subscription_required" => return Err(CloudApiError::SubscriptionRequired),
        "cloud_api_not_enabled" => return Err(CloudApiError::NotEnabled),
        _ => return Err(unauthorized(state, None)),
    }

    request.extensions_mut().insert(AuthContext {
        token: String::new(),
        claims: Claims {
            sub: verified.user_id,
            email: None,
            entitlements: vec!["hyprnote_pro".to_string()],
            subscription_status: None,
            trial_end: None,
            has_payment_method: None,
        },
    });

    Ok(next.run(request).await)
}

async fn enforce_user_status(state: &AppState, user_id: &str) -> Result<(), CloudApiError> {
    let verified = state
        .verify_user(user_id)
        .await
        .map_err(|error| CloudApiError::Internal(error.to_string()))?;
    match verified.status.as_str() {
        "ok" => Ok(()),
        "subscription_required" => Err(CloudApiError::SubscriptionRequired),
        "cloud_api_not_enabled" => Err(CloudApiError::NotEnabled),
        _ => Err(unauthorized(state, None)),
    }
}

fn unauthorized(state: &AppState, error: Option<(&str, &str)>) -> CloudApiError {
    CloudApiError::Unauthorized(state.oauth().challenge(error))
}

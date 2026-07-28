use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use hypr_api_auth::{AuthContext, Claims};
use sha2::{Digest, Sha256};

use crate::{CloudApiError, state::AppState};

pub async fn require_cloud_api_key(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, CloudApiError> {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| token.starts_with("anl_") && token.len() == 68)
        .ok_or(CloudApiError::Unauthorized)?;
    let key_hash = Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let verified = state
        .verify_key(&key_hash)
        .await
        .map_err(|error| CloudApiError::Internal(error.to_string()))?
        .ok_or(CloudApiError::Unauthorized)?;

    match verified.status.as_str() {
        "ok" => {}
        "subscription_required" => return Err(CloudApiError::SubscriptionRequired),
        "cloud_api_not_enabled" => return Err(CloudApiError::NotEnabled),
        _ => return Err(CloudApiError::Unauthorized),
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

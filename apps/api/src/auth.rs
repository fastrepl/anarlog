use axum::{extract::Request, middleware::Next, response::Response};

use anlg_api_auth::AuthContext;
pub use anlg_api_auth::{AuthState, require_auth};

const DEVICE_FINGERPRINT_HEADER: &str = "x-device-fingerprint";

pub async fn sentry_and_analytics(mut request: Request, next: Next) -> Response {
    let device_fingerprint = request
        .headers()
        .get(DEVICE_FINGERPRINT_HEADER)
        .and_then(|h| h.to_str().ok())
        .map(String::from);

    if let Some(auth) = request.extensions().get::<AuthContext>() {
        let user_id = auth.claims.sub.clone();
        request
            .extensions_mut()
            .insert(anlg_analytics::AuthenticatedUserId(user_id));
    }

    if let Some(fingerprint) = device_fingerprint {
        request
            .extensions_mut()
            .insert(anlg_analytics::DeviceFingerprint(fingerprint));
    }

    next.run(request).await
}

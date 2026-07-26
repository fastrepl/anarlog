use std::collections::BTreeMap;

use axum::{extract::Request, middleware::Next, response::Response};

use hypr_api_auth::AuthContext;
pub use hypr_api_auth::{AuthState, optional_auth, require_auth};

const DEVICE_FINGERPRINT_HEADER: &str = "x-device-fingerprint";

pub async fn sentry_and_analytics(mut request: Request, next: Next) -> Response {
    let span = tracing::Span::current();
    let device_fingerprint = request
        .headers()
        .get(DEVICE_FINGERPRINT_HEADER)
        .and_then(|h| h.to_str().ok())
        .map(String::from);

    if let Some(auth) = request.extensions().get::<AuthContext>() {
        sentry::configure_scope(|scope| {
            scope.set_user(Some(sentry::User {
                id: Some(auth.claims.sub.clone()),
                email: auth.claims.email.clone(),
                username: Some(auth.claims.sub.clone()),
                ..Default::default()
            }));
            scope.set_tag("enduser.id", &auth.claims.sub);
            if let Some(fingerprint) = device_fingerprint.as_deref() {
                scope.set_tag("enduser.pseudo.id", fingerprint);
            }

            let mut ctx = BTreeMap::new();
            ctx.insert(
                "hyprnote.enduser.entitlements".into(),
                sentry::protocol::Value::Array(
                    auth.claims
                        .entitlements
                        .iter()
                        .map(|e| sentry::protocol::Value::String(e.clone()))
                        .collect(),
                ),
            );
            scope.set_context(
                "hyprnote.enduser.claims",
                sentry::protocol::Context::Other(ctx),
            );
        });

        let user_id = auth.claims.sub.clone();
        span.record("enduser.id", user_id.as_str());
        request
            .extensions_mut()
            .insert(hypr_analytics::AuthenticatedUserId(user_id));
    }

    if let Some(fingerprint) = device_fingerprint {
        span.record("enduser.pseudo.id", fingerprint.as_str());
        request
            .extensions_mut()
            .insert(hypr_analytics::DeviceFingerprint(fingerprint));
    }

    next.run(request).await
}

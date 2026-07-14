use axum::{
    Extension, Json, Router,
    extract::State,
    http::{HeaderValue, header},
    routing::post,
};
use chrono::{SecondsFormat, TimeDelta, Utc};
use hypr_api_auth::AuthContext;
use serde::{Deserialize, Serialize};
use utoipa::OpenApi;

use crate::error::{Result, SyncError};
use crate::state::AppState;

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloudsyncCredentials {
    database_id: String,
    token: String,
    expires_at: String,
    workspace_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateTokenRequest<'a> {
    name: &'static str,
    user_id: &'a str,
    expires_at: &'a str,
}

#[derive(Deserialize)]
struct CreateTokenEnvelope {
    data: CreateTokenResponse,
}

#[derive(Deserialize)]
struct CreateTokenResponse {
    token: String,
}

#[derive(OpenApi)]
#[openapi(paths(create_credentials), components(schemas(CloudsyncCredentials)))]
pub struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/token", post(create_credentials))
        .with_state(state)
}

#[utoipa::path(
    post,
    path = "/token",
    tag = "sync",
    responses(
        (status = 200, description = "Short-lived CloudSync credentials", body = CloudsyncCredentials),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription required"),
        (status = 502, description = "Credential issuer unavailable")
    )
)]
async fn create_credentials(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<CloudsyncCredentials>,
)> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }

    let ttl = i64::try_from(state.config.token_ttl_seconds)
        .map_err(|_| SyncError::Internal("CloudSync token TTL is too large".to_string()))?;
    let ttl = TimeDelta::try_seconds(ttl)
        .ok_or_else(|| SyncError::Internal("CloudSync token TTL is too large".to_string()))?;
    let expires_at = Utc::now()
        .checked_add_signed(ttl)
        .ok_or_else(|| SyncError::Internal("CloudSync token expiry is invalid".to_string()))?
        .to_rfc3339_opts(SecondsFormat::Secs, true);
    let response = state
        .client
        .post(format!("{}/v2/tokens", state.config.project_url))
        .bearer_auth(&state.config.token_issuer_api_key)
        .json(&CreateTokenRequest {
            name: "anarlog-cloudsync",
            user_id: &auth.claims.sub,
            expires_at: &expires_at,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "SQLite Cloud token request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "SQLite Cloud token request was rejected");
        return Err(SyncError::Upstream);
    }
    let response: CreateTokenEnvelope = response.json().await.map_err(|error| {
        tracing::warn!(%error, "SQLite Cloud token response was invalid");
        SyncError::Upstream
    })?;
    if response.data.token.trim().is_empty() {
        return Err(SyncError::Upstream);
    }

    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(CloudsyncCredentials {
            database_id: state.config.database_id,
            token: response.data.token,
            expires_at,
            workspace_id: auth.claims.sub,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use axum::{Extension, body::Body, body::to_bytes, http::Request, http::StatusCode};
    use hypr_api_auth::{AuthContext, Claims};
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header, method, path},
    };

    use super::*;
    use crate::SyncConfig;

    fn test_router(server: &MockServer, api_key: &str, entitlements: &[&str]) -> Router {
        router(AppState::new(SyncConfig {
            project_url: server.uri(),
            token_issuer_api_key: api_key.to_string(),
            database_id: "database-id".to_string(),
            token_ttl_seconds: 60,
        }))
        .layer(Extension(AuthContext {
            token: "supabase-token".to_string(),
            claims: Claims {
                sub: "user-123".to_string(),
                email: None,
                entitlements: entitlements
                    .iter()
                    .map(|entitlement| (*entitlement).to_string())
                    .collect(),
                subscription_status: None,
                trial_end: None,
                has_payment_method: None,
            },
        }))
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn mints_token_for_verified_supabase_subject() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v2/tokens"))
            .and(header("authorization", "Bearer issuer-key"))
            .and(body_partial_json(json!({
                "name": "anarlog-cloudsync",
                "userId": "user-123"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "token": "sqlite-token" }
            })))
            .mount(&server)
            .await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(Request::post("/token").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = response_json(response).await;
        assert_eq!(body["databaseId"], "database-id");
        assert_eq!(body["token"], "sqlite-token");
        assert_eq!(body["workspaceId"], "user-123");
        assert!(body["expiresAt"].as_str().unwrap().ends_with('Z'));
    }

    #[tokio::test]
    async fn rejects_users_without_pro_entitlement() {
        let cases: &[&[&str]] = &[&[], &["hyprnote_lite"]];

        for entitlements in cases {
            let server = MockServer::start().await;
            let response = test_router(&server, "issuer-key", entitlements)
                .oneshot(Request::post("/token").body(Body::empty()).unwrap())
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            let body = response_json(response).await;
            assert_eq!(body["error"]["code"], "subscription_required");
            assert!(server.received_requests().await.unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn upstream_failure_is_redacted() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v2/tokens"))
            .respond_with(ResponseTemplate::new(403).set_body_string("secret upstream detail"))
            .mount(&server)
            .await;

        let response = test_router(&server, "issuer-secret", &["hyprnote_pro"])
            .oneshot(Request::post("/token").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = response_json(response).await.to_string();
        assert!(!body.contains("issuer-secret"));
        assert!(!body.contains("upstream detail"));
    }
}

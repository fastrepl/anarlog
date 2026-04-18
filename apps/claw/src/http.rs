use std::net::SocketAddr;

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderName, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

const HEADER_USER_ID: HeaderName = HeaderName::from_static("x-exedev-userid");
const HEADER_EMAIL: HeaderName = HeaderName::from_static("x-exedev-email");
const HEADER_TOKEN_CTX: HeaderName = HeaderName::from_static("x-exedev-token-ctx");

/// Identity injected by the exe.dev HTTPS proxy on every authenticated request.
///
/// Claw trusts these because the proxy only forwards them for requests that
/// validated a signed exe0/exe1 token. If a request arrives without them,
/// treat it as unauthenticated and reject.
#[derive(Debug, Clone)]
#[allow(dead_code)] // `ctx` is threaded into logs only for now; routes will read it.
pub struct ExeDevIdentity {
    pub user_id: String,
    pub email: Option<String>,
    pub ctx: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct ControlPlaneState {
    // Room for wiring in: orchestrator handle, channel sender, etc.
    // Kept empty for now so this module can compile standalone and we can
    // stitch dependencies in as claw grows.
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Deserialize)]
struct MessageRequest {
    #[serde(default)]
    text: String,
    #[serde(default)]
    metadata: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct MessageResponse {
    accepted: bool,
    user_id: String,
}

pub fn router(state: ControlPlaneState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/message", post(message))
        .with_state(state)
}

pub async fn serve(addr: SocketAddr, state: ControlPlaneState) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "claw http control plane listening");
    axum::serve(listener, router(state)).await?;
    Ok(())
}

async fn health() -> impl IntoResponse {
    Json(HealthResponse { status: "ok" })
}

async fn message(
    State(_state): State<ControlPlaneState>,
    headers: HeaderMap,
    Json(req): Json<MessageRequest>,
) -> Result<Json<MessageResponse>, (StatusCode, String)> {
    let identity = extract_identity(&headers).ok_or((
        StatusCode::UNAUTHORIZED,
        "missing X-ExeDev-UserID header (are you going through the exe.dev proxy?)".to_string(),
    ))?;

    tracing::info!(
        user_id = %identity.user_id,
        email = identity.email.as_deref().unwrap_or(""),
        text_len = req.text.len(),
        metadata_is_object = req.metadata.is_object(),
        "received message"
    );

    // TODO: forward to zeroclaw orchestrator once we have a handle into it.
    Ok(Json(MessageResponse {
        accepted: true,
        user_id: identity.user_id,
    }))
}

fn extract_identity(headers: &HeaderMap) -> Option<ExeDevIdentity> {
    let user_id = headers.get(&HEADER_USER_ID)?.to_str().ok()?.to_string();
    if user_id.is_empty() {
        return None;
    }
    let email = headers
        .get(&HEADER_EMAIL)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let ctx = headers
        .get(&HEADER_TOKEN_CTX)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| serde_json::from_str(s).ok());
    Some(ExeDevIdentity {
        user_id,
        email,
        ctx,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, header},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_ok() {
        let app = router(ControlPlaneState::default());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn message_requires_identity_header() {
        let app = router(ControlPlaneState::default());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/message")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"text":"hi"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn message_accepts_with_identity() {
        let app = router(ControlPlaneState::default());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/message")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header("x-exedev-userid", "u-123")
                    .header("x-exedev-email", "u@example.com")
                    .body(Body::from(r#"{"text":"hi","metadata":{"k":1}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}

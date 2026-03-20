use axum::{
    Router,
    body::Body,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use reqwest::Client;

const UPSTREAM_BASE: &str = "https://104.198.76.3";

#[derive(Clone)]
pub struct CactusProxyConfig {
    pub api_key: String,
}

#[derive(Clone)]
struct AppState {
    config: CactusProxyConfig,
    client: Client,
}

pub fn router(config: CactusProxyConfig) -> Router {
    let state = AppState {
        config,
        client: Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .expect("failed to build reqwest client"),
    };

    Router::new()
        .route("/text", post(proxy_text))
        .route("/vlm", post(proxy_vlm))
        .route("/transcribe", post(proxy_transcribe))
        .with_state(state)
}

async fn proxy(state: &AppState, path: &str, body: bytes::Bytes) -> Response {
    let url = format!("{UPSTREAM_BASE}/api/v1/{path}");

    let result = state
        .client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("X-API-Key", &state.config.api_key)
        .body(body)
        .send()
        .await;

    match result {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            match resp.bytes().await {
                Ok(bytes) => Response::builder()
                    .status(status)
                    .header("Content-Type", "application/json")
                    .body(Body::from(bytes))
                    .unwrap(),
                Err(e) => {
                    tracing::error!(error = %e, "cactus_proxy_body_read_failed");
                    StatusCode::BAD_GATEWAY.into_response()
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, upstream_url = %url, "cactus_proxy_request_failed");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

async fn proxy_text(State(state): State<AppState>, body: bytes::Bytes) -> Response {
    proxy(&state, "text", body).await
}

async fn proxy_vlm(State(state): State<AppState>, body: bytes::Bytes) -> Response {
    proxy(&state, "vlm", body).await
}

async fn proxy_transcribe(State(state): State<AppState>, body: bytes::Bytes) -> Response {
    proxy(&state, "transcribe", body).await
}

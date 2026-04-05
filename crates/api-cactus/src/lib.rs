use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use reqwest::Client;

const DEFAULT_UPSTREAM_BASE: &str = "https://104.198.76.3";
const DEVICE_FINGERPRINT_HEADER: &str = "x-device-fingerprint";

#[derive(Clone)]
pub struct CactusProxyConfig {
    pub api_key: String,
    pub upstream_base: Option<String>,
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

async fn proxy(
    state: &AppState,
    path: &str,
    headers: &mut HeaderMap,
    body: bytes::Bytes,
) -> Response {
    let fingerprint = headers
        .remove(DEVICE_FINGERPRINT_HEADER)
        .and_then(|v| v.to_str().ok().map(String::from));

    if let Some(fp) = fingerprint {
        tracing::info!(enduser.pseudo.id = %fp, "cactus_proxy_{}", path);
    }

    let base = state
        .config
        .upstream_base
        .as_deref()
        .unwrap_or(DEFAULT_UPSTREAM_BASE);
    let url = format!("{base}/api/v1/{path}");

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

async fn proxy_text(
    State(state): State<AppState>,
    mut headers: HeaderMap,
    body: bytes::Bytes,
) -> Response {
    proxy(&state, "text", &mut headers, body).await
}

async fn proxy_vlm(
    State(state): State<AppState>,
    mut headers: HeaderMap,
    body: bytes::Bytes,
) -> Response {
    proxy(&state, "vlm", &mut headers, body).await
}

async fn proxy_transcribe(
    State(state): State<AppState>,
    mut headers: HeaderMap,
    body: bytes::Bytes,
) -> Response {
    proxy(&state, "transcribe", &mut headers, body).await
}

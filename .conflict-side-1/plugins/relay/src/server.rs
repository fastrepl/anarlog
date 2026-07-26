use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    extract::{State, ws::WebSocketUpgrade},
    response::{IntoResponse, Response},
    routing::get,
};
use tauri::{AppHandle, Runtime};
use tower_http::cors::{Any, CorsLayer};

use crate::relay::PendingResults;

struct AppState<R: Runtime> {
    app: AppHandle<R>,
    pending: PendingResults,
}

pub async fn run<R: Runtime>(
    app: AppHandle<R>,
    addr: SocketAddr,
    pending: PendingResults,
) -> Result<(), std::io::Error> {
    let state = Arc::new(AppState { app, pending });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let router = Router::new()
        .route("/ws", get(ws_upgrade::<R>))
        .route("/health", get(|| async { "ok" }))
        .fallback(get(proxy_to_vite))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;
    tracing::info!("[relay] listening on http://{local_addr}");

    axum::serve(listener, router).await?;
    Ok(())
}

async fn ws_upgrade<R: Runtime>(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState<R>>>,
) -> impl IntoResponse {
    let app = state.app.clone();
    let pending = state.pending.clone();
    ws.on_upgrade(move |socket| crate::relay::handle_ws(socket, app, pending))
}

async fn proxy_to_vite(req: axum::extract::Request) -> Response {
    let path = req.uri().path();
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let url = format!("http://localhost:1422{path}{query}");

    match do_proxy(&url).await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::warn!("[relay] proxy error for {path}: {e}");
            (
                axum::http::StatusCode::BAD_GATEWAY,
                format!("proxy error: {e}"),
            )
                .into_response()
        }
    }
}

async fn do_proxy(url: &str) -> Result<Response, String> {
    let resp = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = axum::http::StatusCode::from_u16(resp.status().as_u16())
        .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);

    let mut builder = axum::http::Response::builder().status(status);
    for (key, value) in resp.headers() {
        builder = builder.header(key, value);
    }

    let body = resp.bytes().await.map_err(|e| e.to_string())?;
    builder
        .body(axum::body::Body::from(body))
        .map_err(|e| e.to_string())
}

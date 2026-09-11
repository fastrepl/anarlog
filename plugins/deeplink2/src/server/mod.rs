use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::response::Html;
use axum::routing::get;
use tauri::Manager;
use tauri_specta::Event;
use tokio::sync::Notify;

use crate::types::{DeepLink, DeepLinkEvent};
pub use anlg_deeplink_core::{parse_callback, render_html, render_html_from_callback};

const CALLBACK_SERVER_TTL: Duration = Duration::from_secs(600);

struct ServerHandle {
    shutdown: Arc<Notify>,
    join_handle: tokio::task::JoinHandle<()>,
}

pub struct CallbackServerState {
    servers: Mutex<HashMap<u16, ServerHandle>>,
    active_port: Mutex<Option<u16>>,
}

impl Default for CallbackServerState {
    fn default() -> Self {
        Self {
            servers: Mutex::new(HashMap::new()),
            active_port: Mutex::new(None),
        }
    }
}

impl CallbackServerState {
    pub fn new() -> Self {
        Self::default()
    }
}

fn emit_deeplink<R: tauri::Runtime, E: std::fmt::Debug>(
    app: &tauri::AppHandle<R>,
    result: Result<DeepLink, E>,
    path: &str,
) {
    match result {
        Ok(deep_link) => {
            tracing::info!(kind = deep_link.path(), "deeplink_emitted");
            if let Err(e) = DeepLinkEvent(deep_link).emit(app) {
                tracing::error!(error = ?e, "deeplink_event_emit_failed");
            }
        }
        Err(e) => {
            tracing::error!(error = ?e, path = %path, "deeplink_parse_failed");
        }
    }
}

async fn handle_request<R: tauri::Runtime>(
    uri: axum::extract::OriginalUri,
    app: tauri::AppHandle<R>,
    shutdown: Arc<Notify>,
    scheme: String,
) -> Html<String> {
    let path = uri.0.path().trim_start_matches('/');
    let query = uri.0.query().unwrap_or("");

    tracing::info!(path = %path, "callback_received");

    let parse_result = parse_callback(path, query);
    let html = render_html_from_callback(path, query, &scheme);

    emit_deeplink(&app, parse_result, path);
    shutdown.notify_one();

    // Subscription codes bounce through `{scheme}://auth/callback?code=…` so
    // the OS opens the app. Token logins stay focus-only to avoid a second
    // auth callback with the same secrets.
    Html(html)
}

async fn serve<R: tauri::Runtime>(
    listener: tokio::net::TcpListener,
    app: tauri::AppHandle<R>,
    shutdown: Arc<Notify>,
    scheme: String,
    port: u16,
) {
    let handler = {
        let app = app.clone();
        let shutdown = shutdown.clone();

        move |uri: axum::extract::OriginalUri| {
            let app = app.clone();
            let shutdown = shutdown.clone();
            let scheme = scheme.clone();
            async move { handle_request(uri, app, shutdown, scheme).await }
        }
    };

    let router = axum::Router::new().fallback(get(handler));

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = shutdown.notified() => {},
                _ = tokio::time::sleep(CALLBACK_SERVER_TTL) => {
                    tracing::info!(port, "callback_server_expired");
                },
            }
        })
        .await
        .ok();

    let state = app.state::<CallbackServerState>();
    if let Ok(mut servers) = state.servers.lock() {
        servers.remove(&port);
    }
    if let Ok(mut active) = state.active_port.lock()
        && *active == Some(port)
    {
        *active = None;
    }
}

pub async fn start<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scheme: String,
    port: Option<u16>,
) -> Result<u16, String> {
    stop(app.clone()).await?;

    let shutdown = Arc::new(Notify::new());

    let bind_addr = match port {
        Some(port) => format!("127.0.0.1:{port}"),
        None => "127.0.0.1:0".to_string(),
    };

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| format!("failed to bind: {e}"))?;

    let port = listener
        .local_addr()
        .map_err(|e| format!("failed to get addr: {e}"))?
        .port();

    let join_handle = tokio::spawn(serve(listener, app.clone(), shutdown.clone(), scheme, port));

    tracing::info!(port, "callback_server_started");

    let state = app.state::<CallbackServerState>();
    state.servers.lock().unwrap().insert(
        port,
        ServerHandle {
            shutdown,
            join_handle,
        },
    );
    *state.active_port.lock().unwrap() = Some(port);

    Ok(port)
}

pub async fn stop<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    let port = {
        let state = app.state::<CallbackServerState>();
        state.active_port.lock().unwrap().take()
    };

    if let Some(port) = port {
        let handle = {
            let state = app.state::<CallbackServerState>();
            state.servers.lock().unwrap().remove(&port)
        };

        if let Some(handle) = handle {
            handle.shutdown.notify_one();
            let _ = handle.join_handle.await;
            tracing::info!(port, "callback_server_stopped");
        }
    }

    Ok(())
}

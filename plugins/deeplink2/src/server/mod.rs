use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use askama::Template;
use axum::response::Html;
use axum::routing::get;
use tauri::Manager;
use tauri_specta::Event;
use tokio::sync::Notify;

use crate::types::{DeepLink, DeepLinkEvent};

const CALLBACK_SERVER_TTL: Duration = Duration::from_secs(600);

#[derive(Template)]
#[template(path = "callback.html")]
struct CallbackTemplate {
    deeplink_url: String,
    is_success: bool,
    title: String,
    description: String,
}

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

pub fn render_html(deep_link: &DeepLink, scheme: &str) -> String {
    let (is_success, title, description) = ui_content(deep_link);
    render_template_html(scheme, is_success, title, description)
}

pub fn render_html_from_callback(path: &str, query: &str, scheme: &str) -> String {
    let parse_result = parse_callback(path, query);
    render_html_from_parse_result(parse_result.as_ref(), scheme)
}

pub fn parse_callback(path: &str, query: &str) -> Result<DeepLink, crate::Error> {
    let path = path.trim_start_matches('/');
    let pseudo_url = if query.is_empty() {
        format!("local://{path}")
    } else {
        format!("local://{path}?{query}")
    };

    DeepLink::from_str(&pseudo_url)
}

fn render_html_from_parse_result<E>(parse_result: Result<&DeepLink, &E>, scheme: &str) -> String {
    let (is_success, title, description) = parse_result
        .map(ui_content)
        .unwrap_or_else(|_| default_ui_content());
    render_template_html(scheme, is_success, title, description)
}

fn render_template_html(scheme: &str, is_success: bool, title: &str, description: &str) -> String {
    CallbackTemplate {
        deeplink_url: format!("{scheme}://focus"),
        is_success,
        title: title.to_string(),
        description: description.to_string(),
    }
    .render()
    .unwrap_or_default()
}

fn default_ui_content() -> (bool, &'static str, &'static str) {
    (
        false,
        "Something went wrong",
        "Please close this window and try again.",
    )
}

fn ui_content(deep_link: &DeepLink) -> (bool, &'static str, &'static str) {
    match deep_link {
        DeepLink::AuthCallback(_) => (
            true,
            "Signed in successfully",
            "Click the button below to return to the app.",
        ),
        DeepLink::BillingRefresh(_) => (
            true,
            "Subscription updated",
            "Click the button below to return to the app.",
        ),
        DeepLink::IntegrationCallback(s) if s.status == "success" => (
            true,
            "Connected successfully",
            "Click the button below to return to the app.",
        ),
        DeepLink::IntegrationCallback(s) if s.status == "upgrade_required" => (
            false,
            "Upgrade required",
            "You can close this window and upgrade your plan to connect this integration.",
        ),
        DeepLink::IntegrationCallback(_) => (
            false,
            "Connection failed",
            "Something went wrong. Please close this window and try again.",
        ),
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
    let html = render_html_from_parse_result(parse_result.as_ref(), &scheme);

    emit_deeplink(&app, parse_result, path);
    shutdown.notify_one();

    // Focus-only deeplink — no callback payload — so the OS focuses/opens the
    // app without re-emitting a DeepLinkEvent and causing duplicate side effects.
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
) -> Result<u16, String> {
    stop(app.clone()).await?;

    let shutdown = Arc::new(Notify::new());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
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

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

use crate::types::{AuthCallbackSearch, DeepLink, DeepLinkEvent};

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
    render_template_html(scheme, is_success, title, description, Some(deep_link))
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
    let deep_link = parse_result.ok();
    let (is_success, title, description) =
        deep_link.map(ui_content).unwrap_or_else(default_ui_content);
    render_template_html(scheme, is_success, title, description, deep_link)
}

fn render_template_html(
    scheme: &str,
    is_success: bool,
    title: &str,
    description: &str,
    deep_link: Option<&DeepLink>,
) -> String {
    CallbackTemplate {
        deeplink_url: return_to_app_url(scheme, deep_link),
        is_success,
        title: title.to_string(),
        description: description.to_string(),
    }
    .render()
    .unwrap_or_default()
}

fn return_to_app_url(scheme: &str, deep_link: Option<&DeepLink>) -> String {
    if let Some(DeepLink::AuthCallback(search)) = deep_link
        && let Some(url) = subscription_auth_deeplink(scheme, search)
    {
        return url;
    }

    format!("{scheme}://focus")
}

pub(crate) fn subscription_auth_deeplink(
    scheme: &str,
    search: &AuthCallbackSearch,
) -> Option<String> {
    let code = search.code.as_deref()?.trim();
    if code.is_empty() || !search.access_token.is_empty() || !search.refresh_token.is_empty() {
        return None;
    }

    let mut url = url::Url::parse(&format!("{scheme}://auth/callback")).ok()?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("code", code);
        if let Some(state) = search
            .state
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            pairs.append_pair("state", state);
        }
    }
    Some(url.into())
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
        DeepLink::AuthCallback(search)
            if search
                .code
                .as_deref()
                .is_some_and(|code| !code.trim().is_empty())
                && search.access_token.is_empty()
                && search.refresh_token.is_empty() =>
        {
            (
                true,
                "Connected successfully",
                "Returning to Anarlog to finish connecting.",
            )
        }
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
        DeepLink::OnboardingDemoComplete(_) => (
            true,
            "Demo complete",
            "Anarlog is finishing your transcript and creating your summary.",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AuthCallbackSearch;

    fn subscription_search() -> AuthCallbackSearch {
        AuthCallbackSearch {
            code: Some("ac_nf5hq".to_string()),
            state: Some("state-1".to_string()),
            ..AuthCallbackSearch::default()
        }
    }

    #[test]
    fn subscription_code_bounces_through_custom_scheme_deeplink() {
        assert_eq!(
            subscription_auth_deeplink("anarlog", &subscription_search()).as_deref(),
            Some("anarlog://auth/callback?code=ac_nf5hq&state=state-1")
        );
        let html = render_html(&DeepLink::AuthCallback(subscription_search()), "anarlog");
        assert!(html.contains("anarlog://auth/callback?code=ac_nf5hq"));
        assert!(html.contains("state=state-1"));
        assert!(html.contains(r#"id="open-app""#));
        assert!(html.contains(r#"document.getElementById("open-app")?.click()"#));
        assert!(html.contains("Connected successfully"));
        assert!(!html.contains("anarlog://focus"));
    }

    #[test]
    fn token_login_stays_focus_only() {
        let html = render_html(
            &DeepLink::AuthCallback(AuthCallbackSearch {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                code: Some("should-ignore".to_string()),
                ..AuthCallbackSearch::default()
            }),
            "anarlog-dev",
        );
        assert!(html.contains("anarlog-dev://focus"));
        assert!(!html.contains("code=should-ignore"));
        assert!(html.contains("Signed in successfully"));
    }

    #[test]
    fn loopback_query_renders_subscription_deeplink() {
        let html = render_html_from_callback(
            "/auth/callback",
            "code=codex-code&state=s1&scope=openid",
            "anarlog",
        );
        assert!(html.contains("anarlog://auth/callback?code=codex-code"));
        assert!(html.contains("state=s1"));
    }
}

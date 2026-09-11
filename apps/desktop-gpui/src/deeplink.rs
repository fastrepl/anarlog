//! Deep links for the native shell: the `tauri-plugin-deeplink2` +
//! `tauri-plugin-single-instance` pair.
//!
//! The OS hands `anarlog://…` URLs to whichever binary owns the scheme (the
//! Tauri launcher, which forwards its arguments here). A second launch
//! forwards its URLs over a Unix socket to the running instance instead of
//! opening a second window, and a bare relaunch just focuses it. Browser
//! flows that cannot use the custom scheme (the onboarding demo) call back
//! into a short-lived loopback HTTP server, like `startCallbackServer`.

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use anlg_deeplink_core::{DeepLink, IncomingDeepLink, ShareOpenRequest};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader as AsyncBufReader};

/// `callback_server_ttl`: an unanswered callback server goes away on its own.
const CALLBACK_SERVER_TTL: Duration = Duration::from_secs(600);

/// `deep_link_scheme`: the custom URL scheme registered for a bundle.
pub fn scheme(identifier: &str) -> &'static str {
    match identifier {
        "com.hyprnote.stable" | "com.hyprnote.Hyprnote" => "anarlog",
        "com.hyprnote.staging" => "anarlog-staging",
        _ => "anarlog-dev",
    }
}

/// What arrived: a routed deep link, a shared-note open, or a plain
/// `{scheme}://focus` / second launch that only wants the window back.
#[derive(Debug)]
pub enum Incoming {
    DeepLink(DeepLink),
    ShareOpen(ShareOpenRequest),
    Focus,
}

/// `process_url`: parse and classify one URL. Unknown or malformed URLs are
/// logged (redacted) and treated as a focus request so the window still
/// comes forward.
pub fn classify(url: &str) -> Incoming {
    let redacted = anlg_deeplink_core::redact_url(url);
    tracing::info!(url = %redacted, "deeplink_received");
    match IncomingDeepLink::from_str(url) {
        Ok(IncomingDeepLink::Existing(deep_link)) => {
            tracing::info!(path = deep_link.path(), "deeplink_parsed");
            Incoming::DeepLink(deep_link)
        }
        Ok(IncomingDeepLink::ShareOpen(request)) => {
            tracing::info!(path = "/share/open", "deeplink_parsed");
            Incoming::ShareOpen(request)
        }
        Err(error) => {
            tracing::debug!(?error, url = %redacted, "deeplink_parse_failed");
            Incoming::Focus
        }
    }
}

/// Command-line arguments that are URLs: the OS passes the clicked link as
/// the only positional argument.
pub fn urls_from_args(args: &[std::ffi::OsString]) -> Vec<String> {
    args.iter()
        .filter_map(|arg| arg.to_str())
        .filter(|arg| url::Url::parse(arg).is_ok_and(|url| !url.cannot_be_a_base()))
        .map(str::to_string)
        .collect()
}

/// The single-instance socket next to the database.
pub fn socket_path(db_path: &Path) -> PathBuf {
    db_path.with_file_name("gpui.sock")
}

pub enum Claim {
    /// This process owns the socket; URLs from later launches arrive on
    /// the receiver, one per line.
    Primary(Receiver<String>),
    /// Another instance is running and received our URLs; exit.
    Forwarded,
}

/// `tauri_plugin_single_instance`: connect to a running instance and hand it
/// our URLs, or become the instance everyone else forwards to.
#[cfg(unix)]
pub fn claim(socket: &Path, urls: &[String]) -> Claim {
    use std::os::unix::net::{UnixListener, UnixStream};

    if let Ok(mut stream) = UnixStream::connect(socket) {
        let mut payload = urls.join("\n");
        payload.push('\n');
        if stream.write_all(payload.as_bytes()).is_ok() {
            tracing::info!(
                count = urls.len(),
                "forwarded launch to the running instance"
            );
            return Claim::Forwarded;
        }
    }
    // A stale socket from a crashed instance refuses connections.
    let _ = std::fs::remove_file(socket);
    let (sender, receiver) = channel();
    match UnixListener::bind(socket) {
        Ok(listener) => {
            std::thread::Builder::new()
                .name("single-instance".into())
                .spawn(move || accept_forwards(listener, sender))
                .ok();
        }
        Err(error) => {
            tracing::warn!(%error, path = %socket.display(), "single-instance socket unavailable");
        }
    }
    Claim::Primary(receiver)
}

#[cfg(not(unix))]
pub fn claim(_socket: &Path, _urls: &[String]) -> Claim {
    let (_sender, receiver) = channel();
    Claim::Primary(receiver)
}

#[cfg(unix)]
fn accept_forwards(listener: std::os::unix::net::UnixListener, sender: Sender<String>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        let mut lines = Vec::new();
        for line in BufReader::new(stream).lines() {
            let Ok(line) = line else {
                break;
            };
            lines.push(line);
        }
        // An empty payload is a bare relaunch: focus only.
        let urls: Vec<String> = lines.into_iter().filter(|line| !line.is_empty()).collect();
        if urls.is_empty() {
            if sender.send(String::new()).is_err() {
                return;
            }
            continue;
        }
        for url in urls {
            if sender.send(url).is_err() {
                return;
            }
        }
    }
}

/// `CallbackServerState`: at most one loopback server; starting a new one
/// stops the previous.
#[derive(Clone)]
pub struct CallbackServer {
    runtime: tokio::runtime::Handle,
    scheme: &'static str,
    sender: Sender<String>,
    active: Arc<Mutex<Option<Arc<tokio::sync::Notify>>>>,
}

impl CallbackServer {
    pub fn new(
        runtime: tokio::runtime::Handle,
        scheme: &'static str,
        sender: Sender<String>,
    ) -> Self {
        Self {
            runtime,
            scheme,
            sender,
            active: Arc::new(Mutex::new(None)),
        }
    }

    /// `startCallbackServer(scheme, null)`: bind an ephemeral loopback port
    /// whose first request is rendered as the callback page and routed like
    /// a deep link.
    pub async fn start(&self) -> Result<u16, String> {
        self.stop();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|error| format!("failed to bind: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| format!("failed to get addr: {error}"))?
            .port();
        let shutdown = Arc::new(tokio::sync::Notify::new());
        *self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(shutdown.clone());
        let scheme = self.scheme;
        let sender = self.sender.clone();
        let active = self.active.clone();
        self.runtime.spawn(async move {
            tokio::select! {
                _ = serve(listener, scheme, sender, shutdown.clone()) => {}
                _ = shutdown.notified() => {}
                _ = tokio::time::sleep(CALLBACK_SERVER_TTL) => {
                    tracing::info!(port, "callback_server_expired");
                }
            }
            let mut active = active.lock().unwrap_or_else(|error| error.into_inner());
            if active
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &shutdown))
            {
                *active = None;
            }
        });
        tracing::info!(port, "callback_server_started");
        Ok(port)
    }

    /// `start` on the tokio runtime, for callers on the GPUI executor.
    pub fn start_task(&self) -> tokio::task::JoinHandle<Result<u16, String>> {
        let server = self.clone();
        self.runtime.spawn(async move { server.start().await })
    }

    pub fn stop(&self) {
        if let Some(shutdown) = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            shutdown.notify_one();
        }
    }
}

/// Serve requests until one is handled; `handle_request` answers every path
/// (axum's `fallback`) and shuts the server down afterwards.
async fn serve(
    listener: tokio::net::TcpListener,
    scheme: &'static str,
    sender: Sender<String>,
    shutdown: Arc<tokio::sync::Notify>,
) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        if handle_request(stream, scheme, &sender).await {
            shutdown.notify_one();
            return;
        }
    }
}

/// One HTTP/1.x GET: render the callback page for the path and query and
/// route the pseudo URL. Returns whether a request was answered.
async fn handle_request(
    mut stream: tokio::net::TcpStream,
    scheme: &'static str,
    sender: &Sender<String>,
) -> bool {
    let mut reader = AsyncBufReader::new(&mut stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await.is_err() {
        return false;
    }
    // Drain the headers so the browser sees a clean close.
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header).await {
            Ok(0) => break,
            Ok(_) if header == "\r\n" || header == "\n" => break,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    drop(reader);
    let Some(target) = request_line.split_whitespace().nth(1) else {
        return false;
    };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let path = path.trim_start_matches('/');
    tracing::info!(path = %path, "callback_received");

    let html = anlg_deeplink_core::render_html_from_callback(path, query, scheme);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;

    let pseudo_url = if query.is_empty() {
        format!("local://{path}")
    } else {
        format!("local://{path}?{query}")
    };
    let _ = sender.send(pseudo_url);
    true
}

/// `buildWelcomeNoteDemoUrl`: the demo autoplays and reports completion to
/// the loopback server when one is running.
pub fn welcome_demo_url(meeting_link: &str, port: Option<u16>) -> String {
    let Ok(mut url) = url::Url::parse(meeting_link) else {
        return meeting_link.to_string();
    };
    url.query_pairs_mut().append_pair("autojoin", "1");
    if let Some(port) = port {
        url.query_pairs_mut().append_pair(
            "completion_url",
            &format!("http://127.0.0.1:{port}/onboarding-demo/complete"),
        );
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemes_follow_the_bundle_identifier() {
        assert_eq!(scheme("com.hyprnote.stable"), "anarlog");
        assert_eq!(scheme("com.hyprnote.staging"), "anarlog-staging");
        assert_eq!(scheme("com.hyprnote.dev"), "anarlog-dev");
    }

    #[test]
    fn classifies_routed_links_share_opens_and_focus() {
        assert!(matches!(
            classify("anarlog://onboarding-demo/complete"),
            Incoming::DeepLink(DeepLink::OnboardingDemoComplete(_))
        ));
        assert!(matches!(
            classify(
                "anarlog://share/open?mode=account&share_id=40bc9d36-7634-4c48-988f-6a3e301467e7"
            ),
            Incoming::ShareOpen(ShareOpenRequest::Account { .. })
        ));
        assert!(matches!(classify("anarlog-dev://focus"), Incoming::Focus));
        assert!(matches!(classify("not a url"), Incoming::Focus));
    }

    #[test]
    fn only_url_arguments_are_forwarded() {
        let args: Vec<std::ffi::OsString> =
            ["--identifier", "com.hyprnote.dev", "anarlog-dev://focus"]
                .into_iter()
                .map(Into::into)
                .collect();
        assert_eq!(
            urls_from_args(&args),
            vec!["anarlog-dev://focus".to_string()]
        );
    }

    #[test]
    fn welcome_demo_url_matches_the_tauri_builder() {
        assert_eq!(
            welcome_demo_url("https://anarlog.so/onboarding-demo/", None),
            "https://anarlog.so/onboarding-demo/?autojoin=1"
        );
        assert_eq!(
            welcome_demo_url("https://anarlog.so/onboarding-demo/", Some(43210)),
            "https://anarlog.so/onboarding-demo/?autojoin=1&completion_url=http%3A%2F%2F127.0.0.1%3A43210%2Fonboarding-demo%2Fcomplete"
        );
    }

    #[cfg(unix)]
    #[test]
    fn second_launch_forwards_its_urls_to_the_primary() {
        let dir = std::env::temp_dir().join(format!("anlg-si-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let socket = socket_path(&dir.join("app.db"));
        let Claim::Primary(receiver) = claim(&socket, &[]) else {
            panic!("first claim should be primary");
        };
        assert!(matches!(
            claim(&socket, &["anarlog-dev://focus".to_string()]),
            Claim::Forwarded
        ));
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
            "anarlog-dev://focus"
        );
        assert!(matches!(claim(&socket, &[]), Claim::Forwarded));
        assert_eq!(receiver.recv_timeout(Duration::from_secs(5)).unwrap(), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn callback_server_answers_once_and_routes_the_pseudo_url() {
        let (sender, receiver) = channel();
        let server = CallbackServer::new(tokio::runtime::Handle::current(), "anarlog-dev", sender);
        let port = server.start().await.unwrap();

        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        stream
            .write_all(b"GET /onboarding-demo/complete HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut body = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut body)
            .await
            .unwrap();
        let body = String::from_utf8(body).unwrap();
        assert!(body.starts_with("HTTP/1.1 200 OK"));
        assert!(body.contains("Demo complete"));
        assert!(body.contains("anarlog-dev://focus"));
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
            "local://onboarding-demo/complete"
        );
        assert!(matches!(
            classify("local://onboarding-demo/complete"),
            Incoming::DeepLink(DeepLink::OnboardingDemoComplete(_))
        ));

        // The server shut down after the first request.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_err()
        );
    }
}

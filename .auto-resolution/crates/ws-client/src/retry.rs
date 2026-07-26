use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use backon::{ConstantBuilder, Retryable};
use tokio_tungstenite::tungstenite::http::{HeaderMap, HeaderValue};
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

pub type WebSocketRetryCallback = Arc<dyn Fn(WebSocketRetryEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct WebSocketConnectPolicy {
    pub connect_timeout: std::time::Duration,
    pub max_attempts: usize,
    pub retry_delay: std::time::Duration,
}

impl Default for WebSocketConnectPolicy {
    fn default() -> Self {
        Self {
            connect_timeout: std::time::Duration::from_secs(5),
            max_attempts: 3,
            retry_delay: std::time::Duration::from_millis(750),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WebSocketRetryEvent {
    pub attempt: usize,
    pub max_attempts: usize,
    pub error: String,
}

pub(crate) async fn connect_with_retry(
    request: tokio_tungstenite::tungstenite::ClientRequestBuilder,
    policy: &WebSocketConnectPolicy,
    on_retry: Option<&WebSocketRetryCallback>,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    crate::Error,
> {
    let max_attempts = policy.max_attempts.max(1);
    let attempts_made = Arc::new(AtomicUsize::new(0));
    let attempts_ref = attempts_made.clone();

    let result = (|| {
        let request = request.clone();
        let attempts_ref = attempts_ref.clone();
        async move {
            let attempt = attempts_ref.fetch_add(1, Ordering::SeqCst) + 1;
            try_connect(request, policy.connect_timeout, attempt, max_attempts).await
        }
    })
    .retry(
        ConstantBuilder::default()
            .with_delay(policy.retry_delay)
            .with_max_times(max_attempts - 1),
    )
    .when(|e: &crate::Error| e.is_retryable_connect_error())
    .adjust(|e: &crate::Error, dur| {
        if let crate::Error::ConnectFailed {
            retry_after_secs: Some(secs),
            ..
        } = e
        {
            Some(std::time::Duration::from_secs(*secs))
        } else {
            dur
        }
    })
    .notify(|e: &crate::Error, dur| {
        let attempt = attempts_ref.load(Ordering::SeqCst);
        tracing::warn!(
            attempt,
            max_attempts,
            delay_ms = dur.as_millis() as u64,
            "ws_connect_retry: {:?}",
            e
        );
        if let Some(callback) = on_retry {
            callback(WebSocketRetryEvent {
                attempt: attempt + 1,
                max_attempts,
                error: e.to_string(),
            });
        }
    })
    .await;

    match result {
        Ok(stream) => Ok(stream),
        Err(error @ crate::Error::ConnectRetriesExhausted { .. }) => Err(error),
        Err(error) if !error.is_retryable_connect_error() => Err(error),
        Err(error) => {
            let attempts = attempts_made.load(Ordering::SeqCst);
            Err(crate::Error::connect_retries_exhausted(
                attempts,
                error.to_string(),
            ))
        }
    }
}

async fn try_connect(
    req: tokio_tungstenite::tungstenite::ClientRequestBuilder,
    timeout: std::time::Duration,
    attempt: usize,
    max_attempts: usize,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    crate::Error,
> {
    let mut req = req
        .into_client_request()
        .map_err(|error| crate::Error::invalid_request(error.to_string()))?;

    // AWS WAF and similar firewalls reject WebSocket upgrades without a User-Agent.
    if !req.headers().contains_key("user-agent") {
        req.headers_mut().insert(
            "user-agent",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static("ws-client/0.1.0"),
        );
    }

    let redacted_request = format!("{:?}", RedactedRequest(&req));

    tracing::info!(
        attempt,
        max_attempts,
        request = %redacted_request,
        "connect_async"
    );

    let connect_result = tokio::time::timeout(timeout, connect_async(req)).await;
    let (ws_stream, _) = match connect_result {
        Ok(Ok(stream)) => stream,
        Ok(Err(error)) => {
            tracing::error!(
                attempt,
                max_attempts,
                request = %redacted_request,
                error = %error,
                "connect_async_failed"
            );
            return Err(crate::Error::connect_failed(attempt, max_attempts, &error));
        }
        Err(_) => return Err(crate::Error::connect_timeout(attempt, max_attempts)),
    };

    Ok(ws_stream)
}

struct RedactedRequest<'a>(&'a tokio_tungstenite::tungstenite::handshake::client::Request);

impl fmt::Debug for RedactedRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedactedRequest")
            .field("uri", &self.0.uri())
            .field("headers", &RedactedHeaders(self.0.headers()))
            .finish()
    }
}

struct RedactedHeaders<'a>(&'a HeaderMap<HeaderValue>);

impl fmt::Debug for RedactedHeaders<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut entries = self
            .0
            .iter()
            .map(|(name, value)| {
                let key = name.as_str().to_ascii_lowercase();
                let value = if is_sensitive_header(&key) {
                    redact_header_value(&key, value)
                } else {
                    value.to_str().unwrap_or("<non-utf8>").to_string()
                };
                (key, value)
            })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        f.debug_list()
            .entries(
                entries
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.as_str())),
            )
            .finish()
    }
}

fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name,
        "authorization" | "proxy-authorization" | "x-api-key" | "xi-api-key" | "x-gladia-key"
    )
}

fn redact_header_value(name: &str, value: &HeaderValue) -> String {
    let raw = value.to_str().unwrap_or("<non-utf8>");
    if name == "authorization" || name == "proxy-authorization" {
        let mut parts = raw.splitn(2, ' ');
        let scheme = parts.next().unwrap_or_default();
        if parts.next().is_some() && !scheme.is_empty() {
            return format!("{scheme} <redacted>");
        }
    }
    "<redacted>".to_string()
}

#[cfg(test)]
mod tests {
    use tokio_tungstenite::tungstenite::http::{HeaderMap, HeaderValue};

    use super::{RedactedHeaders, redact_header_value};

    #[test]
    fn redacted_headers_redacts_sensitive_values() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Token secret-key"),
        );
        headers.insert("x-api-key", HeaderValue::from_static("secret-api-key"));
        headers.insert("user-agent", HeaderValue::from_static("ws-client/0.1.0"));

        let redacted = format!("{:?}", RedactedHeaders(&headers));

        assert!(redacted.contains("(\"authorization\", \"Token <redacted>\")"));
        assert!(redacted.contains("(\"x-api-key\", \"<redacted>\")"));
        assert!(redacted.contains("(\"user-agent\", \"ws-client/0.1.0\")"));
    }

    #[test]
    fn redact_authorization_preserves_scheme() {
        let value = HeaderValue::from_static("Bearer abc123");
        assert_eq!(
            redact_header_value("authorization", &value),
            "Bearer <redacted>"
        );
    }
}

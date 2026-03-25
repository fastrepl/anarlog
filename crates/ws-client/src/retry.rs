use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use backon::{ConstantBuilder, Retryable};
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

pub type WebSocketRetryCallback = std::sync::Arc<dyn Fn(WebSocketRetryEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct WebSocketConnectPolicy {
    pub connect_timeout: Duration,
    pub max_attempts: usize,
    pub retry_delay: Duration,
}

impl Default for WebSocketConnectPolicy {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            max_attempts: 3,
            retry_delay: Duration::from_millis(750),
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
    let attempt_count = AtomicUsize::new(0);

    let result = (|| {
        let request = request.clone();
        async {
            let attempt = attempt_count.fetch_add(1, Ordering::SeqCst) + 1;
            try_connect(request, policy.connect_timeout, attempt, max_attempts).await
        }
    })
    .retry(
        ConstantBuilder::default()
            .with_delay(policy.retry_delay)
            .with_max_times(max_attempts - 1),
    )
    .when(|e: &crate::Error| e.is_retryable_connect_error())
    .adjust(|err, dur| match err {
        crate::Error::ConnectFailed {
            retry_after_secs: Some(secs),
            ..
        } => Some(Duration::from_secs(*secs)),
        _ => dur,
    })
    .notify(|err, _dur| {
        tracing::error!("ws_connect_failed: {:?}", err);
        if let Some(callback) = on_retry {
            callback(WebSocketRetryEvent {
                attempt: attempt_count.load(Ordering::SeqCst) + 1,
                max_attempts,
                error: err.to_string(),
            });
        }
    })
    .await;

    match result {
        Ok(stream) => Ok(stream),
        Err(error) => {
            let attempts = attempt_count.load(Ordering::SeqCst);
            if attempts >= max_attempts {
                Err(crate::Error::connect_retries_exhausted(
                    attempts,
                    error.to_string(),
                ))
            } else {
                Err(error)
            }
        }
    }
}

async fn try_connect(
    req: tokio_tungstenite::tungstenite::ClientRequestBuilder,
    timeout: Duration,
    attempt: usize,
    max_attempts: usize,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    crate::Error,
> {
    let mut req = req
        .into_client_request()
        .map_err(|error| crate::Error::invalid_request(error.to_string()))?;

    if !req.headers().contains_key("user-agent") {
        req.headers_mut().insert(
            "user-agent",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static("ws-client/0.1.0"),
        );
    }

    tracing::info!("connect_async: {}", loggable_uri(req.uri()));

    let connect_result = tokio::time::timeout(timeout, connect_async(req)).await;
    let (ws_stream, _) = match connect_result {
        Ok(Ok(stream)) => stream,
        Ok(Err(error)) => return Err(crate::Error::connect_failed(attempt, max_attempts, &error)),
        Err(_) => return Err(crate::Error::connect_timeout(attempt, max_attempts)),
    };

    Ok(ws_stream)
}

fn loggable_uri(uri: &tokio_tungstenite::tungstenite::http::Uri) -> String {
    let mut parts = uri.clone().into_parts();
    if let Some(path_and_query) = parts.path_and_query.as_ref() {
        parts.path_and_query = path_and_query.path().parse().ok();
    }

    tokio_tungstenite::tungstenite::http::Uri::from_parts(parts)
        .map(|uri| uri.to_string())
        .unwrap_or_else(|_| uri.path().to_string())
}

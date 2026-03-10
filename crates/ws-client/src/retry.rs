use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

pub type WebSocketRetryCallback = std::sync::Arc<dyn Fn(WebSocketRetryEvent) + Send + Sync>;

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
    let mut attempts_made = 0usize;
    let mut last_error: Option<crate::Error> = None;

    for attempt in 1..=max_attempts {
        attempts_made = attempt;
        match try_connect(
            request.clone(),
            policy.connect_timeout,
            attempt,
            max_attempts,
        )
        .await
        {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                tracing::error!("ws_connect_failed: {:?}", error);

                if !error.is_retryable_connect_error() {
                    return Err(error);
                }

                if attempt >= max_attempts {
                    last_error = Some(error);
                    break;
                }

                if let Some(callback) = on_retry {
                    callback(WebSocketRetryEvent {
                        attempt: attempt + 1,
                        max_attempts,
                        error: error.to_string(),
                    });
                }

                last_error = Some(error);
                tokio::time::sleep(policy.retry_delay).await;
            }
        }
    }

    match last_error {
        Some(error @ crate::Error::ConnectRetriesExhausted { .. }) => Err(error),
        Some(error) => Err(crate::Error::connect_retries_exhausted(
            attempts_made,
            error.to_string(),
        )),
        None => Err(crate::Error::connect_retries_exhausted(
            attempts_made,
            "connect failed",
        )),
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
    let req = req
        .into_client_request()
        .map_err(|error| crate::Error::invalid_request(error.to_string()))?;

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

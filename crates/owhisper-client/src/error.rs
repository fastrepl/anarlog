#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("audio processing error: {0}")]
    AudioProcessing(String),
    #[error("provider failure: {message}")]
    ProviderFailure {
        message: String,
        retryable: bool,
        status: Option<reqwest::StatusCode>,
    },
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    HttpMiddleware(#[from] reqwest_middleware::Error),
    #[error(transparent)]
    Task(#[from] tokio::task::JoinError),
    #[error("unexpected response status {status}: {body}")]
    UnexpectedStatus {
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("websocket error: {0}")]
    WebSocket(String),
}

impl Error {
    pub fn provider_failure(message: impl Into<String>, retryable: bool) -> Self {
        Self::ProviderFailure {
            message: message.into(),
            retryable,
            status: None,
        }
    }

    pub fn provider_failure_with_status(
        status: reqwest::StatusCode,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self::ProviderFailure {
            message: message.into(),
            retryable,
            status: Some(status),
        }
    }
}

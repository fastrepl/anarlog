use std::sync::Arc;

#[derive(Debug, thiserror::Error, Clone)]
pub enum Error {
    #[error("supabase auth session missing")]
    SessionMissing,
    #[error("invalid api key header value")]
    InvalidApiKey(#[source] Arc<reqwest::header::InvalidHeaderValue>),
    #[error(transparent)]
    Network(#[from] Arc<reqwest::Error>),
    #[error("supabase auth request failed ({status}): {message}")]
    Api {
        status: u16,
        code: Option<String>,
        message: String,
    },
    #[error("invalid session payload")]
    InvalidSession(#[source] Arc<serde_json::Error>),
}

impl From<reqwest::header::InvalidHeaderValue> for Error {
    fn from(error: reqwest::header::InvalidHeaderValue) -> Self {
        Self::InvalidApiKey(Arc::new(error))
    }
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::Network(Arc::new(error))
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::InvalidSession(Arc::new(error))
    }
}

impl Error {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::Api { status, .. } => {
                matches!(*status, 502 | 503 | 504 | 520 | 521 | 522 | 523 | 524 | 530)
            }
            _ => false,
        }
    }

    pub fn is_fatal(&self) -> bool {
        match self {
            Self::SessionMissing => true,
            Self::Api {
                code: Some(code), ..
            } => matches!(
                code.as_str(),
                "refresh_token_not_found" | "refresh_token_already_used" | "session_expired"
            ),
            _ => false,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

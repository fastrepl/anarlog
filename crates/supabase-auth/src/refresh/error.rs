#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error("missing refresh token")]
    MissingRefreshToken,
    #[error("invalid api key header value")]
    InvalidApiKey(#[from] reqwest::header::InvalidHeaderValue),
    #[error("invalid session payload")]
    InvalidSession(#[source] serde_json::Error),
    #[error("supabase auth request failed ({status}): {message}")]
    Auth { status: u16, message: String },
}

pub type Result<T> = std::result::Result<T, Error>;

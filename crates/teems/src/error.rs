use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP client error: {0}")]
    Http(Box<dyn std::error::Error + Send + Sync>),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Teams API error: {code} - {message}")]
    TeamsApi { code: String, message: String },
}

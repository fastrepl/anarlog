#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("url parse error: {0}")]
    Url(#[from] url::ParseError),
    #[error("missing api key")]
    MissingApiKey,
    #[error("invalid api key")]
    InvalidApiKey,
    #[error("api error (status {status}): {message}")]
    Api { status: u16, message: String },
}

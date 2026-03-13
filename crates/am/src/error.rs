#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Request(#[from] reqwest::Error),

    #[error(transparent)]
    HyprFile(#[from] hypr_file::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("Could not find home directory")]
    NoHomeDir,

    #[error("Server returned error: {status} - {message}")]
    ServerError { status: String, message: String },

    #[error("Invalid API key format: must start with 'ax_'")]
    InvalidApiKey,

    #[error("Unexpected response from server")]
    UnexpectedResponse,

    #[error("Tar file not found")]
    TarFileNotFound,
}

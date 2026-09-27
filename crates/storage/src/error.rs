#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("data directory not available")]
    DataDirUnavailable,
    #[error("path must be absolute")]
    PathNotAbsolute,
    #[error("path contains invalid UTF-8")]
    PathNotValidUtf8,
    #[error("path exists but is not a directory")]
    PathIsNotDirectory,
    #[error("{0}")]
    SecureStorage(String),
}

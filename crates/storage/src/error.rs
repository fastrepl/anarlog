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
    #[error("cannot move vault to a subdirectory of the current location")]
    VaultBaseIsSubdirectory,
    #[error("cannot move vault to a parent directory of the current location")]
    VaultBaseIsParent,
}

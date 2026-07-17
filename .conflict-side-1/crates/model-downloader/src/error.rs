#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("model not downloaded: {0}")]
    ModelNotDownloaded(String),
    #[error("download failed: {0}")]
    DownloadFailed(#[from] hypr_file::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("operation failed: {0}")]
    OperationFailed(String),
    #[error("finalize failed: {0}")]
    FinalizeFailed(String),
    #[error("no download URL available for model: {0}")]
    NoDownloadUrl(String),
    #[error("delete failed: {0}")]
    DeleteFailed(String),
}

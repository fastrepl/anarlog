pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("attachment metadata is invalid")]
    InvalidMetadata,
    #[error("attachment transfer state is invalid")]
    InvalidTransferState,
    #[error("attachment transfer range is invalid")]
    InvalidRange,
    #[error("attachment is unavailable locally")]
    LocalAttachmentUnavailable,
    #[error("attachment workspace key is unavailable")]
    WorkspaceKeyUnavailable,
    #[error("attachment download URL is invalid")]
    InvalidDownloadUrl,
    #[error("attachment download failed")]
    Download(#[source] reqwest::Error),
    #[error("attachment download was incomplete")]
    IncompleteDownload,
    #[error("attachment checksum does not match")]
    ChecksumMismatch,
    #[error("attachment cache is unavailable")]
    CacheUnavailable,
    #[error("attachment delete guard changed during commit")]
    DeleteGuardChanged,
    #[error("attachment transfer was cancelled")]
    Cancelled,
    #[error("attachment database operation failed")]
    Database(#[source] sqlx::Error),
    #[error("attachment filesystem operation failed")]
    Io(#[source] std::io::Error),
    #[error("attachment encryption operation failed")]
    E2ee(#[source] hypr_e2ee::AttachmentBlobError),
    #[error("attachment vault is unavailable")]
    Vault,
}

impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<hypr_e2ee::AttachmentBlobError> for Error {
    fn from(error: hypr_e2ee::AttachmentBlobError) -> Self {
        Self::E2ee(error)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error("Error while reading file: {0}")]
    FileIOError(#[from] std::io::Error),
    #[error("Download cancelled")]
    Cancelled,
    #[error("Other error: {0}")]
    OtherError(String),
}

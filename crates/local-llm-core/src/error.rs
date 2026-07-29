#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    AnlgFileError(#[from] anlg_file::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    LmStudioError(#[from] anlg_lmstudio::Error),
    #[error("Model not downloaded")]
    ModelNotDownloaded,
    #[error("Other error: {0}")]
    Other(String),
}

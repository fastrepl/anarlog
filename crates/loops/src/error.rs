#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
}

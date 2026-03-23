#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to create VAD session")]
    VadSessionCreationFailed,
    #[error("Failed to process audio")]
    VadProcessingFailed(String),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to create VAD session: {0}")]
    VadSessionCreationFailed(String),
    #[error("Unsupported sample rate: expected 16000 Hz, got {0} Hz")]
    UnsupportedSampleRate(u32),
    #[error("Invalid VAD config: {0}")]
    InvalidConfig(String),
    #[error("Failed to process audio: {0}")]
    VadProcessingFailed(String),
}

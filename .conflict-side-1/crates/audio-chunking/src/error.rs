#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to create speech chunking session: {0}")]
    SessionCreationFailed(String),
    #[error("Unsupported sample rate: expected 16000 Hz, got {0} Hz")]
    UnsupportedSampleRate(u32),
    #[error("Invalid speech chunking config: {0}")]
    InvalidConfig(String),
    #[error("Failed to process speech chunking audio: {0}")]
    ProcessingFailed(String),
}

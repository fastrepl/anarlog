#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to load model: {0}")]
    Load(Box<dyn std::error::Error + Send + Sync>),
    #[error("model not registered: {0}")]
    ModelNotRegistered(String),
    #[error("model file not found: {0}")]
    ModelFileNotFound(String),
    #[error("no default model configured")]
    NoDefaultModel,
    #[error("worker task panicked")]
    WorkerPanicked,
}

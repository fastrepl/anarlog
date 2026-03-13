#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Cactus(#[from] hypr_cactus::Error),
    #[error("model not registered: {0}")]
    ModelNotRegistered(String),
    #[error("model file not found: {0}")]
    ModelFileNotFound(String),
    #[error("no default model configured")]
    NoDefaultModel,
    #[error("worker task panicked")]
    WorkerPanicked,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Ort(#[from] ort::Error),

    #[error("failed to read model from path: {0}")]
    ReadModelFromPath(#[from] std::io::Error),
}

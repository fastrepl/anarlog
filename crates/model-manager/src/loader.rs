use std::{fmt, path::Path, sync::Arc};

pub trait ModelLoader: Send + Sync + 'static {
    type Error: fmt::Display + fmt::Debug + Send + 'static;

    fn load(path: &Path) -> Result<Self, Self::Error>
    where
        Self: Sized;
}

#[derive(Debug, thiserror::Error)]
pub enum Error<E: fmt::Display + fmt::Debug> {
    #[error("model not registered: {0}")]
    ModelNotRegistered(String),
    #[error("model file not found: {0}")]
    ModelFileNotFound(String),
    #[error("no default model configured")]
    NoDefaultModel,
    #[error("worker task panicked")]
    WorkerPanicked,
    #[error(transparent)]
    Load(E),
}

pub enum TryGetResult<M> {
    Ready(Arc<M>),
    Loading,
    NotRegistered,
    Failed(String),
}

pub enum ModelStatus<M> {
    Ready(Arc<M>),
    Loading,
    Idle,
    NotRegistered,
    Failed(String),
}

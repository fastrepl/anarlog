use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to initialize model: {0}")]
    Init(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("null pointer from cactus FFI")]
    NullPointer,
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Nul(#[from] std::ffi::NulError),
}

impl Error {
    pub fn is_invalid_request(&self) -> bool {
        matches!(self, Self::InvalidRequest(_))
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

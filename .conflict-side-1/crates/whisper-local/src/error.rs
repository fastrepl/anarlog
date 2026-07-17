use serde::{Serialize, ser::Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("model_not_found")]
    ModelNotFound,
    #[cfg(feature = "actual")]
    #[error(transparent)]
    LocalWhisperError(#[from] whisper_rs::WhisperError),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

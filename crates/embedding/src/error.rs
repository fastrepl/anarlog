use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "onnx")]
    #[error(transparent)]
    AnlgOnnxError(#[from] anlg_onnx::Error),

    #[cfg(feature = "onnx")]
    #[error(transparent)]
    OrtError(#[from] anlg_onnx::ort::Error),

    #[cfg(feature = "onnx")]
    #[error(transparent)]
    ShapeError(#[from] anlg_onnx::ndarray::ShapeError),

    #[error("knf error: {0}")]
    KnfError(String),

    #[error("samples are empty")]
    EmptyInput,

    #[error("mask length ({mask_len}) must match samples length ({samples_len})")]
    MaskLengthMismatch { mask_len: usize, samples_len: usize },

    #[error("audio is too short for embedding")]
    TooShort,

    #[error("embedding model returned non-finite values")]
    NonFiniteEmbedding,

    #[error("missing output tensor: {0}")]
    MissingOutput(String),

    #[error("internal error: {0}")]
    Internal(&'static str),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

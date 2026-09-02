use serde::{Serialize, ser::Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    AnlgOnnxError(#[from] anlg_onnx::Error),
    #[error(transparent)]
    OrtError(#[from] anlg_onnx::ort::Error),
    #[error(transparent)]
    ShapeError(#[from] anlg_onnx::ndarray::ShapeError),
    #[error(transparent)]
    EmbeddingError(#[from] anlg_embedding::Error),
    #[error("empty row in outputs")]
    EmptyRowError,
    #[error("segmentation window must hold {expected} samples, got {actual}")]
    WindowLength { expected: usize, actual: usize },
    #[error("diarization cancelled")]
    Cancelled,
    #[error("audio read failed: {0}")]
    AudioRead(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

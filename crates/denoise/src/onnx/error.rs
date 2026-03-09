use serde::{Serialize, ser::Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    HyprOnnxError(#[from] hypr_onnx::Error),

    #[error(transparent)]
    OrtError(#[from] hypr_onnx::ort::Error),

    #[error(transparent)]
    FftError(#[from] realfft::FftError),

    #[error(transparent)]
    ShapeError(#[from] hypr_onnx::ndarray::ShapeError),

    #[error("Missing output tensor: {0}")]
    MissingOutput(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

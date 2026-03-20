#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "onnx")]
    #[error(transparent)]
    Onnx(#[from] hypr_onnx::Error),
    #[cfg(feature = "onnx")]
    #[error(transparent)]
    Ort(#[from] hypr_onnx::ort::Error),
    #[cfg(feature = "onnx")]
    #[error(transparent)]
    Shape(#[from] hypr_onnx::ndarray::ShapeError),
    #[error("sample rate mismatch: expected {expected}Hz, got {actual}Hz")]
    SampleRateMismatch { expected: u32, actual: u32 },
    #[error("invalid segmenter config `{field}`: {reason}")]
    InvalidConfiguration { field: &'static str, reason: String },
    #[error("empty output row")]
    EmptyOutputRow,
}

pub type Result<T> = std::result::Result<T, Error>;

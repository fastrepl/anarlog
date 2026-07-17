mod error;

pub use error::*;

#[cfg(feature = "onnx")]
pub mod onnx;

#[cfg(feature = "onnx")]
pub use onnx::{EMBEDDING_DIM, EmbeddingConfig, EmbeddingExtractor, SAMPLE_RATE_HZ};

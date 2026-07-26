mod error;
pub use error::*;

use ort::{
    Result,
    session::{Session, builder::GraphOptimizationLevel},
};

pub use ndarray;
pub use ort;

pub fn load_model_from_bytes(bytes: &[u8]) -> Result<Session, Error> {
    Ok(Session::builder()?
        .with_intra_threads(1)?
        .with_inter_threads(1)?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .commit_from_memory(bytes)?)
}

pub fn load_model_from_path(path: impl AsRef<std::path::Path>) -> Result<Session, Error> {
    let bytes = std::fs::read(path)?;
    load_model_from_bytes(&bytes)
}

#[cfg(feature = "async-source")]
mod async_source;
mod driver;

#[cfg(feature = "async-source")]
pub use async_source::*;
pub use driver::RubatoChunkResampler;
pub use rubato::{Async, FixedAsync, PolynomialDegree, Resampler};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ResampleError(#[from] rubato::ResampleError),
    #[error(transparent)]
    ResamplerConstructionError(#[from] rubato::ResamplerConstructionError),
}

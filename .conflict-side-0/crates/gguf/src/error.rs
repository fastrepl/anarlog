use std::result;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid GGUF magic number")]
    InvalidMagic,

    #[error("Unsupported GGUF version: {0}")]
    UnsupportedVersion(u32),

    #[error("Unsupported metadata value type: {0}")]
    UnsupportedValueType(u32),

    #[error("Invalid UTF-8 sequence")]
    InvalidUtf8,
}

pub type Result<T> = result::Result<T, Error>;

use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Format error: {0}")]
    Fmt(#[from] fmt::Error),
    #[error(
        "BigInt type is forbidden by current config. Configure BigIntExportBehavior to handle i64/u64/i128/u128."
    )]
    BigIntForbidden,
    #[error("Invalid type name: '{0}'")]
    InvalidTypeName(String),
}

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to load config: {0}")]
    ConfigLoad(String),
    #[error("failed to parse config: {0}")]
    ConfigParse(String),
    #[error("unsupported config version: {0}")]
    UnsupportedVersion(u8),
}

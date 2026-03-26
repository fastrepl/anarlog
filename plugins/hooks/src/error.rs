use serde::{Serialize, ser::Serializer};
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

impl From<hypr_hooks::Error> for Error {
    fn from(e: hypr_hooks::Error) -> Self {
        match e {
            hypr_hooks::Error::ConfigLoad(s) => Error::ConfigLoad(s),
            hypr_hooks::Error::ConfigParse(s) => Error::ConfigParse(s),
            hypr_hooks::Error::UnsupportedVersion(v) => Error::UnsupportedVersion(v),
        }
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

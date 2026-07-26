use serde::{Serialize, ser::Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("libsql error: {0}")]
    LibsqlError(#[from] libsql::Error),
    #[error("serde::de error: {0}")]
    SerdeDeError(#[from] serde::de::value::Error),
    #[error("serde_json error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("chrono parse error: {0}")]
    ChronoParseError(String),
    #[error("invalid database config: {0}")]
    InvalidDatabaseConfig(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

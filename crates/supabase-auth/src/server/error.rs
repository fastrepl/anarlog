use serde::{Serialize, ser::Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing authorization header")]
    MissingAuthHeader,
    #[error("invalid authorization header")]
    InvalidAuthHeader,
    #[error("failed to fetch JWKS")]
    JwksFetchFailed,
    #[error("invalid token")]
    InvalidToken,
    #[error("missing entitlement: {0}")]
    MissingEntitlement(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

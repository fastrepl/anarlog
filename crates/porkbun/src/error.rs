use serde::{Serialize, ser::Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("API error (status {0}): {1}")]
    Api(u16, String),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error("missing api key")]
    MissingApiKey,
    #[error("missing secret api key")]
    MissingSecretApiKey,
    #[error("invalid api base url")]
    InvalidApiBase,
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

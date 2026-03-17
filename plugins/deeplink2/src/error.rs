use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid deep link URL: {0}")]
    InvalidUrl(String),
    #[error("unknown deep link path: {0}")]
    UnknownPath(String),
    #[error("url parse error: {0}")]
    UrlParse(#[from] url::ParseError),
    #[error("query decode error: {0}")]
    QueryDecode(#[from] serde_qs::Error),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

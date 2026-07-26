use serde::{Serialize, ser::Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("posthog error: {0}")]
    PosthogError(String),
}

impl From<posthog_rs::Error> for Error {
    fn from(e: posthog_rs::Error) -> Self {
        Error::PosthogError(e.to_string())
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

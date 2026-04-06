use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("reminder access denied")]
    ReminderAccessDenied,
    #[error("reminder not found")]
    ReminderNotFound,
    #[error("reminder list not found")]
    ReminderListNotFound,
    #[error("invalid date range")]
    InvalidDateRange,
    #[error("objective-c exception: {0}")]
    ObjectiveCException(String),
    #[error("xpc connection failed")]
    XpcConnectionFailed,
    #[error("transform error: {0}")]
    TransformError(String),
    #[error("fetch timeout")]
    FetchTimeout,
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

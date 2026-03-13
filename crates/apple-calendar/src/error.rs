use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("calendar access denied")]
    CalendarAccessDenied,
    #[error("contacts access denied")]
    ContactsAccessDenied,
    #[error("event not found")]
    EventNotFound,
    #[error("calendar not found")]
    CalendarNotFound,
    #[error("invalid date range")]
    InvalidDateRange,
    #[error("objective-c exception: {0}")]
    ObjectiveCException(String),
    #[error("xpc connection failed")]
    XpcConnectionFailed,
    #[error("transform error: {0}")]
    TransformError(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
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

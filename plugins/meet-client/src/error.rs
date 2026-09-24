use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("meet client is not enabled in this build")]
    Disabled,
    #[error("a meet meeting is already active")]
    AlreadyActive,
    #[error("no meet meeting is active")]
    NotActive,
    #[error("invalid google meet url")]
    InvalidMeetingUrl,
    #[error("observation from unexpected window")]
    UnexpectedWindow,
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

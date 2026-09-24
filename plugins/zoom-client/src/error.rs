use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("zoom client is not enabled in this build")]
    Disabled,
    #[error("a zoom meeting is already active")]
    AlreadyActive,
    #[error("no zoom meeting is active")]
    NotActive,
    #[error("invalid display name")]
    InvalidDisplayName,
    #[error(transparent)]
    Bridge(#[from] anlg_meeting_capture::MeetingSdkBridgeError),
    #[error(transparent)]
    Sidecar(#[from] tauri_plugin_sidecar2::Error),
    #[error("sidecar error: {0}")]
    Shell(#[from] tauri_plugin_shell::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

use serde::{Serialize, ser::Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    TauriError(#[from] tauri::Error),
    #[error("failed to receive result from the main thread")]
    MainThreadRecvFailed,
    #[error("monitor not found")]
    MonitorNotFound,
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

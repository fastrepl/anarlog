use serde::{Serialize, ser::Serializer};

pub use hypr_listener_core::DegradedError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    HyprAudioError(#[from] hypr_audio::Error),
    #[error(transparent)]
    CpalDevicesError(#[from] hypr_audio::cpal::DevicesError),
    #[error(transparent)]
    LocalSttError(#[from] tauri_plugin_local_stt::Error),
    #[error("no session")]
    NoneSession,
    #[error("start session failed")]
    StartSessionFailed,
    #[error("stop session failed")]
    StopSessionFailed,
    #[error("actor not found {0}")]
    ActorNotFound(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

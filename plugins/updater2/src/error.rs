use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Store2(#[from] tauri_plugin_store2::Error),
    #[error(transparent)]
    Updater(#[from] tauri_plugin_updater::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("cache path unavailable")]
    CachePathUnavailable,
    #[error("cached update not found")]
    CachedUpdateNotFound,
    #[error("update not available")]
    UpdateNotAvailable,
    #[error("version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },
    #[error("failed to determine current app path")]
    FailedToDetermineCurrentAppPath,
    #[error("failed to schedule installed app launch at {path}: {details}")]
    FailedToScheduleInstalledAppLaunch { path: String, details: String },
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Store2(#[from] tauri_plugin_store2::Error),
    #[error(transparent)]
    Updater(#[from] tauri_plugin_updater::Error),
    #[error(transparent)]
    Download(#[from] updater_core::Error),
    #[error(transparent)]
    Config(#[from] serde_json::Error),
    #[error(transparent)]
    Version(#[from] semver::Error),
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
    #[error("cached update {version} is not newer than current {current}")]
    UpdateNotNewer { version: String, current: String },
    #[error("failed to determine current app path")]
    FailedToDetermineCurrentAppPath,
    #[error(
        "The update needs system packages apt can't install, so Anarlog stays on its current version ({0}). Run `sudo apt update`, then try the update again."
    )]
    DebDependencies(String),
    #[error("The update failed to install: {0}")]
    DebInstall(String),
    #[error("failed to schedule installed app launch at {path}: {details}")]
    FailedToScheduleInstalledAppLaunch { path: String, details: String },
    #[error(
        "Updates are paused until your notes finish moving from your custom folder ({0}). Restart Anarlog to retry moving them, or contact support if this keeps happening."
    )]
    UnfinishedVaultMove(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

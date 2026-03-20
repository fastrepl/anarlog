use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    AmError(#[from] hypr_am::Error),
    #[error(transparent)]
    HyprFileError(#[from] hypr_file::Error),
    #[error(transparent)]
    ShellError(#[from] tauri_plugin_shell::Error),
    #[error(transparent)]
    Sidecar2Error(#[from] tauri_plugin_sidecar2::Error),
    #[error(transparent)]
    ModelDownloaderError(#[from] hypr_model_downloader::Error),
    #[error("Model not downloaded")]
    ModelNotDownloaded,
    #[error("Server start failed {0}")]
    ServerStartFailed(String),
    #[error("Server stop failed {0}")]
    ServerStopFailed(String),
    #[error("Supervisor not found")]
    SupervisorNotFound,
    #[error("AM API key not set")]
    AmApiKeyNotSet,
    #[error("Internal server only supports Whisper models")]
    UnsupportedModelType,
    #[error("Model delete failed: {0}")]
    ModelDeleteFailed(String),
    #[error("Model unpack failed: {0}")]
    ModelUnpackFailed(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

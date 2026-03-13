use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    CoreError(#[from] hypr_local_llm_core::Error),
    #[error(transparent)]
    StoreError(#[from] tauri_plugin_store2::Error),
    #[error(transparent)]
    ModelDownloaderError(#[from] hypr_model_downloader::Error),
    #[error("Other error: {0}")]
    Other(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

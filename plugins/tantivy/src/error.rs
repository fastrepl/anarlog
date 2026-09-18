use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Index(#[from] anlg_search_index::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    #[error(transparent)]
    Settings(#[from] tauri_plugin_settings::Error),
}

impl Error {
    /// The collection is not registered yet (startup ordering).
    pub fn is_collection_not_found(&self) -> bool {
        matches!(
            self,
            Error::Index(anlg_search_index::Error::CollectionNotFound(_))
        )
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

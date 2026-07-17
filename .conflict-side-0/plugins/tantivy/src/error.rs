use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Tantivy(#[from] tantivy::TantivyError),
    #[error(transparent)]
    QueryParser(#[from] tantivy::query::QueryParserError),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    #[error(transparent)]
    Settings(#[from] tauri_plugin_settings::Error),
    #[error("Index not initialized")]
    IndexNotInitialized,
    #[error("Collection not found: {0}")]
    CollectionNotFound(String),
    #[error("Document not found: {0}")]
    DocumentNotFound(String),
    #[error("Invalid document type: {0}")]
    InvalidDocumentType(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

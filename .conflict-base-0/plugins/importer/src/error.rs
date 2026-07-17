use crate::types::ImportSourceKind;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("granola error: {0}")]
    Granola(#[from] hypr_granola::error::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("db parser error: {0}")]
    DbParser(#[from] legacy_db_parser::Error),

    #[error("import source not found: {0:?}")]
    SourceNotFound(ImportSourceKind),

    #[error("import source not available: {0}")]
    SourceNotAvailable(String),

    #[error("{operation} timed out after {seconds} seconds")]
    Timeout {
        operation: &'static str,
        seconds: u64,
    },

    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),

    #[error("settings error: {0}")]
    Settings(#[from] tauri_plugin_settings::Error),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error("chrono parse error: {0}")]
    ChronoParse(#[from] chrono::ParseError),
}

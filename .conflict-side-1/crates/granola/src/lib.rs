pub mod api;
pub mod cache;
pub mod error;
pub mod fs;
pub mod importer;
pub mod markdown;
pub mod prosemirror;
pub mod transcript;

use crate::api::GranolaClient;
use crate::cache::read_cache;
use crate::error::{Error, Result};
use crate::fs::{write_notes, write_transcripts};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct NotesConfig {
    pub supabase_path: PathBuf,
    pub output_dir: PathBuf,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct TranscriptsConfig {
    pub cache_path: PathBuf,
    pub output_dir: PathBuf,
}

pub fn default_supabase_path() -> PathBuf {
    dirs::config_dir()
        .map(|config| config.join("Granola/supabase.json"))
        .unwrap_or_else(|| PathBuf::from("supabase.json"))
}

pub async fn export_notes(config: &NotesConfig) -> Result<usize> {
    let supabase_content = std::fs::read(&config.supabase_path).map_err(Error::SupabaseFileRead)?;

    let client = GranolaClient::new(&supabase_content, config.timeout)?;
    let documents = client.get_documents().await?;

    write_notes(&documents, &config.output_dir)
}

pub fn export_transcripts(config: &TranscriptsConfig) -> Result<usize> {
    let cache_data = read_cache(&config.cache_path)?;

    write_transcripts(
        &cache_data.documents,
        &cache_data.transcripts,
        &config.output_dir,
    )
}

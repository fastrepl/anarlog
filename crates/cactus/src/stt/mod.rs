mod batch;
mod result;
mod stream;
mod transcriber;
mod whisper;

pub use result::TranscriptionResult;
pub use stream::{TranscribeEvent, TranscriptionSession, transcribe_stream};
pub use transcriber::{CloudConfig, StreamResult, Transcriber};

use hypr_language::Language;

pub fn constrain_to(languages: &[Language]) -> Option<Language> {
    match languages {
        [] => None,
        [single] => Some(single.clone()),
        [first, ..] => {
            tracing::info!(
                ?languages,
                selected = ?first,
                "multi-language constraint unsupported by cactus FFI; using first language"
            );
            Some(first.clone())
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TranscribeOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_chunk_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation_threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_vocabulary: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vocabulary_boost: Option<f32>,
}

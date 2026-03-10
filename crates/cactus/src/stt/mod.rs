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

pub(crate) fn merge_transcribe_options(
    options: &TranscribeOptions,
    defaults: &TranscribeOptions,
) -> TranscribeOptions {
    let mut resolved = options.clone();

    if resolved.custom_vocabulary.is_none() {
        resolved.custom_vocabulary = defaults.custom_vocabulary.clone();
    }
    if resolved.vocabulary_boost.is_none() {
        resolved.vocabulary_boost = defaults.vocabulary_boost;
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::{TranscribeOptions, merge_transcribe_options};

    #[test]
    fn uses_model_default_custom_vocabulary_when_request_does_not_set_one() {
        let resolved = merge_transcribe_options(
            &TranscribeOptions::default(),
            &TranscribeOptions {
                custom_vocabulary: Some(vec!["Cactus".into(), "HIPAA".into()]),
                vocabulary_boost: Some(3.0),
                ..Default::default()
            },
        );

        assert_eq!(
            resolved.custom_vocabulary,
            Some(vec!["Cactus".to_string(), "HIPAA".to_string()])
        );
        assert_eq!(resolved.vocabulary_boost, Some(3.0));
    }

    #[test]
    fn request_custom_vocabulary_overrides_model_default() {
        let resolved = merge_transcribe_options(
            &TranscribeOptions {
                custom_vocabulary: Some(vec!["Parakeet".into()]),
                vocabulary_boost: Some(1.5),
                ..Default::default()
            },
            &TranscribeOptions {
                custom_vocabulary: Some(vec!["Cactus".into()]),
                vocabulary_boost: Some(3.0),
                ..Default::default()
            },
        );

        assert_eq!(
            resolved.custom_vocabulary,
            Some(vec!["Parakeet".to_string()])
        );
        assert_eq!(resolved.vocabulary_boost, Some(1.5));
    }
}

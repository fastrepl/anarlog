mod batch;

use openai_transcription::batch::AudioModel;

use crate::providers::Provider;

use super::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct OpenAIAdapter;

impl OpenAIAdapter {
    pub fn language_support_batch(_languages: &[hypr_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }

    pub fn is_supported_languages_batch(languages: &[hypr_language::Language]) -> bool {
        Self::language_support_batch(languages).is_supported()
    }

    pub(crate) fn resolve_batch_model(model: Option<&str>) -> AudioModel {
        let default = Provider::OpenAI.default_batch_model();

        match model {
            Some(value) if crate::providers::is_meta_model(value) => {
                default.parse().expect("invalid_default_openai_batch_model")
            }
            Some(value) => value
                .parse()
                .unwrap_or_else(|_| default.parse().expect("invalid_default_openai_batch_model")),
            None => default.parse().expect("invalid_default_openai_batch_model"),
        }
    }

    pub fn supports_progressive_batch_model(model: Option<&str>) -> bool {
        matches!(
            Self::resolve_batch_model(model),
            AudioModel::Gpt4oTranscribe
                | AudioModel::Gpt4oMiniTranscribe
                | AudioModel::Gpt4oMiniTranscribe20251215
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_openai_host() {
        assert!(Provider::OpenAI.is_host("api.openai.com"));
        assert!(Provider::OpenAI.is_host("openai.com"));
        assert!(!Provider::OpenAI.is_host("api.deepgram.com"));
    }

    #[test]
    fn resolve_batch_model_defaults_to_diarize() {
        assert_eq!(
            OpenAIAdapter::resolve_batch_model(None),
            AudioModel::Gpt4oTranscribeDiarize
        );
    }

    #[test]
    fn progressive_batch_only_supports_non_diarized_gpt_models() {
        assert!(OpenAIAdapter::supports_progressive_batch_model(Some(
            "gpt-4o-transcribe"
        )));
        assert!(OpenAIAdapter::supports_progressive_batch_model(Some(
            "gpt-4o-mini-transcribe"
        )));
        assert!(!OpenAIAdapter::supports_progressive_batch_model(Some(
            "gpt-4o-transcribe-diarize"
        )));
        assert!(!OpenAIAdapter::supports_progressive_batch_model(Some(
            "whisper-1"
        )));
        assert!(!OpenAIAdapter::supports_progressive_batch_model(None));
    }
}

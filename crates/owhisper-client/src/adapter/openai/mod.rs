mod batch;
mod live;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use openai_transcription::batch::AudioModel;

use crate::providers::Provider;

use super::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct OpenAIAdapter {
    live_state: Arc<Mutex<OpenAILiveState>>,
}

#[derive(Default)]
struct OpenAILiveState {
    input_sample_rate: u32,
    fallback_end_ms: u64,
    item_order: VecDeque<String>,
    items: HashMap<String, OpenAILiveItem>,
}

#[derive(Default)]
struct OpenAILiveItem {
    start_ms: Option<u64>,
    end_ms: Option<u64>,
    transcript: String,
    completed_transcript: Option<String>,
}

impl OpenAIAdapter {
    pub fn for_live_sample_rate(input_sample_rate: u32) -> Self {
        Self {
            live_state: Arc::new(Mutex::new(OpenAILiveState {
                input_sample_rate,
                ..Default::default()
            })),
        }
    }

    pub fn language_support_live(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }

    pub fn language_support_batch(_languages: &[anlg_language::Language]) -> LanguageSupport {
        Self::language_support_live(_languages)
    }

    pub fn is_supported_languages_live(languages: &[anlg_language::Language]) -> bool {
        Self::language_support_live(languages).is_supported()
    }

    pub fn is_supported_languages_batch(languages: &[anlg_language::Language]) -> bool {
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
            AudioModel::GptTranscribe
                | AudioModel::Gpt4oTranscribe
                | AudioModel::Gpt4oMiniTranscribe
                | AudioModel::Gpt4oMiniTranscribe20251215
        )
    }

    pub(crate) fn build_ws_url_from_base(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        if api_base.is_empty() {
            return (
                Provider::OpenAI
                    .default_ws_url()
                    .parse()
                    .expect("invalid_default_ws_url"),
                vec![("intent".to_string(), "transcription".to_string())],
            );
        }

        if let Some(proxy_result) = super::build_proxy_ws_url(api_base) {
            return proxy_result;
        }

        let parsed: url::Url = api_base.parse().expect("invalid_api_base");
        let mut existing_params = super::extract_query_params(&parsed);

        if !existing_params.iter().any(|(key, _)| key == "intent") {
            existing_params.push(("intent".to_string(), "transcription".to_string()));
        }

        let url = super::build_url_with_scheme(
            &parsed,
            Provider::OpenAI.default_ws_host(),
            Provider::OpenAI.ws_path(),
            true,
        );

        (url, existing_params)
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
    fn resolve_batch_model_defaults_to_gpt_transcribe() {
        assert_eq!(
            OpenAIAdapter::resolve_batch_model(None),
            AudioModel::GptTranscribe
        );
    }

    #[test]
    fn progressive_batch_only_supports_non_diarized_gpt_models() {
        assert!(OpenAIAdapter::supports_progressive_batch_model(Some(
            "gpt-transcribe"
        )));
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
        assert!(OpenAIAdapter::supports_progressive_batch_model(None));
    }

    #[test]
    fn build_ws_url_adds_transcription_intent() {
        let (url, params) = OpenAIAdapter::build_ws_url_from_base("");

        assert_eq!(url.as_str(), "wss://api.openai.com/v1/realtime");
        assert_eq!(
            params,
            vec![("intent".to_string(), "transcription".to_string())]
        );
    }

    #[test]
    fn build_ws_url_preserves_proxy_provider() {
        let (url, params) =
            OpenAIAdapter::build_ws_url_from_base("https://api.anarlog.so?provider=openai");

        assert_eq!(url.as_str(), "wss://api.anarlog.so/listen");
        assert_eq!(params, vec![("provider".to_string(), "openai".to_string())]);
    }

    #[test]
    fn build_ws_url_preserves_custom_port() {
        let (url, params) =
            OpenAIAdapter::build_ws_url_from_base("https://stt.example.com:8443/v1");

        assert_eq!(url.as_str(), "wss://stt.example.com:8443/v1/realtime");
        assert_eq!(
            params,
            vec![("intent".to_string(), "transcription".to_string())]
        );
    }
}

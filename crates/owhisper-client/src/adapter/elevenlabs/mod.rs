mod batch;
pub mod error;
mod language;
mod live;

use crate::providers::Provider;
use serde::Deserialize;

use super::LanguageSupport;

#[derive(Clone, Default)]
pub struct ElevenLabsAdapter;

impl ElevenLabsAdapter {
    pub fn language_support_live(languages: &[hypr_language::Language]) -> LanguageSupport {
        LanguageSupport::min(languages.iter().map(language::single_language_support))
    }

    pub fn language_support_batch(languages: &[hypr_language::Language]) -> LanguageSupport {
        Self::language_support_live(languages)
    }

    pub fn is_supported_languages_live(languages: &[hypr_language::Language]) -> bool {
        Self::language_support_live(languages).is_supported()
    }

    pub fn is_supported_languages_batch(languages: &[hypr_language::Language]) -> bool {
        Self::language_support_batch(languages).is_supported()
    }

    pub(crate) fn build_ws_url_from_base(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        super::build_ws_url_from_base_with(Provider::ElevenLabs, api_base, |parsed| {
            super::build_url_with_scheme(
                parsed,
                Provider::ElevenLabs.default_api_host(),
                Provider::ElevenLabs.ws_path(),
                true,
            )
        })
    }

    pub(crate) fn batch_api_url(api_base: &str) -> String {
        if api_base.is_empty() {
            return format!(
                "https://{}/v1/speech-to-text",
                Provider::ElevenLabs.default_api_host()
            );
        }

        let parsed: url::Url = api_base.parse().expect("invalid_api_base");
        super::build_url_with_scheme(
            &parsed,
            Provider::ElevenLabs.default_api_host(),
            "/v1/speech-to-text",
            false,
        )
        .to_string()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ElevenLabsWord {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
    #[serde(default, rename = "type")]
    pub word_type: Option<String>,
    #[serde(default)]
    pub speaker_id: Option<String>,
}

pub(super) fn documented_language_codes() -> Vec<&'static str> {
    let mut codes = Vec::new();
    codes.extend_from_slice(language::EXCELLENT_LANGS);
    codes.extend_from_slice(language::HIGH_LANGS);
    codes.extend_from_slice(language::GOOD_LANGS);
    codes.extend_from_slice(language::MODERATE_LANGS);
    codes.extend_from_slice(language::NO_DATA_LANGS);
    codes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ws_url_from_base() {
        let cases = [
            (
                "",
                "wss://api.elevenlabs.io/v1/speech-to-text/realtime",
                vec![],
            ),
            (
                "https://api.elevenlabs.io",
                "wss://api.elevenlabs.io/v1/speech-to-text/realtime",
                vec![],
            ),
            (
                "https://api.hyprnote.com?provider=elevenlabs",
                "wss://api.hyprnote.com/listen",
                vec![("provider", "elevenlabs")],
            ),
            (
                "http://localhost:8787/listen?provider=elevenlabs",
                "ws://localhost:8787/listen",
                vec![("provider", "elevenlabs")],
            ),
        ];

        for (input, expected_url, expected_params) in cases {
            let (url, params) = ElevenLabsAdapter::build_ws_url_from_base(input);
            assert_eq!(url.as_str(), expected_url, "input: {}", input);
            assert_eq!(
                params,
                expected_params
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<Vec<_>>(),
                "input: {}",
                input
            );
        }
    }

    #[test]
    fn test_is_host() {
        assert!(Provider::ElevenLabs.matches_url("https://api.elevenlabs.io"));
        assert!(Provider::ElevenLabs.matches_url("https://api.elevenlabs.io/v1"));
        assert!(!Provider::ElevenLabs.matches_url("https://api.deepgram.com"));
        assert!(!Provider::ElevenLabs.matches_url("https://api.assemblyai.com"));
    }

    #[test]
    fn test_batch_api_url_empty_uses_default() {
        let url = ElevenLabsAdapter::batch_api_url("");
        assert_eq!(url, "https://api.elevenlabs.io/v1/speech-to-text");
    }

    #[test]
    fn test_batch_api_url_custom() {
        let url = ElevenLabsAdapter::batch_api_url("https://custom.elevenlabs.io");
        assert_eq!(url, "https://custom.elevenlabs.io/v1/speech-to-text");
    }
}

mod batch;
mod live;

use crate::providers::Provider;

use super::{LanguageQuality, LanguageSupport};

const SUPPORTED_LANGUAGES: &[&str] = &[
    "en", "zh", "hi", "es", "ar", "fr", "pt", "ru", "de", "ja", "ko", "it", "nl",
];

#[derive(Clone)]
pub struct MistralAdapter {
    word_counter: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl Default for MistralAdapter {
    fn default() -> Self {
        Self {
            word_counter: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }
}

impl MistralAdapter {
    fn is_language_supported(lang: &hypr_language::Language) -> bool {
        lang.matches_any_code(SUPPORTED_LANGUAGES)
    }

    fn language_support_impl(languages: &[hypr_language::Language]) -> LanguageSupport {
        if languages.is_empty() {
            return LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            };
        }

        if languages.iter().all(Self::is_language_supported) {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        } else {
            LanguageSupport::NotSupported
        }
    }

    pub fn language_support_live(languages: &[hypr_language::Language]) -> LanguageSupport {
        Self::language_support_impl(languages)
    }

    pub fn language_support_batch(languages: &[hypr_language::Language]) -> LanguageSupport {
        Self::language_support_impl(languages)
    }

    pub fn is_supported_languages_live(languages: &[hypr_language::Language]) -> bool {
        Self::language_support_live(languages).is_supported()
    }

    pub fn is_supported_languages_batch(languages: &[hypr_language::Language]) -> bool {
        Self::language_support_batch(languages).is_supported()
    }

    pub(crate) fn build_ws_url_from_base(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        super::build_ws_url_from_base_with(Provider::Mistral, api_base, |parsed| {
            let host = parsed
                .host_str()
                .unwrap_or(Provider::Mistral.default_ws_host());
            let mut url: url::Url = format!("wss://{}{}", host, Provider::Mistral.ws_path())
                .parse()
                .expect("invalid_ws_url");
            super::set_scheme_from_host(&mut url);
            url
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ws_url_from_base_empty() {
        let (url, params) = MistralAdapter::build_ws_url_from_base("");
        assert_eq!(
            url.as_str(),
            "wss://api.mistral.ai/v1/audio/transcriptions/realtime"
        );
        assert!(params.is_empty());
    }

    #[test]
    fn test_build_ws_url_from_base_proxy() {
        let (url, params) =
            MistralAdapter::build_ws_url_from_base("https://api.hyprnote.com?provider=mistral");
        assert_eq!(url.as_str(), "wss://api.hyprnote.com/listen");
        assert_eq!(
            params,
            vec![("provider".to_string(), "mistral".to_string())]
        );
    }

    #[test]
    fn test_is_mistral_host() {
        assert!(Provider::Mistral.is_host("api.mistral.ai"));
        assert!(Provider::Mistral.is_host("mistral.ai"));
        assert!(!Provider::Mistral.is_host("api.openai.com"));
    }
}

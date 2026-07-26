mod batch;
mod language;
mod live;

use super::{LanguageQuality, LanguageSupport};

const DEFAULT_API_HOST: &str = "api.smallest.ai";
const API_PATH: &str = "/waves/v1/pulse/get_text";
const DEFAULT_MODEL: &str = "pulse";

#[derive(Clone, Default)]
pub struct SmallestAIAdapter;

impl SmallestAIAdapter {
    fn supports_model(model: Option<&str>) -> bool {
        match model {
            None => true,
            Some(model) if crate::providers::is_meta_model(model) => true,
            Some(model) => model.eq_ignore_ascii_case(DEFAULT_MODEL),
        }
    }

    fn is_language_supported(language: &hypr_language::Language) -> bool {
        language.matches_any_code(language::SUPPORTED_LANGUAGES)
    }

    fn language_support_impl(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        if !Self::supports_model(model) {
            return LanguageSupport::NotSupported;
        }

        if languages.is_empty() || languages.iter().all(Self::is_language_supported) {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        } else {
            LanguageSupport::NotSupported
        }
    }

    pub fn language_support_live(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        Self::language_support_impl(languages, model)
    }

    pub fn language_support_batch(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        Self::language_support_impl(languages, model)
    }

    pub fn is_supported_languages_live(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        Self::language_support_live(languages, model).is_supported()
    }

    pub fn is_supported_languages_batch(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        Self::language_support_batch(languages, model).is_supported()
    }

    pub(crate) fn language_query_value(languages: &[hypr_language::Language]) -> String {
        match languages {
            [language] => language.iso639().code().to_string(),
            _ => "multi".to_string(),
        }
    }

    pub(crate) fn build_ws_url_from_base(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        let default_url = || {
            (
                format!("wss://{DEFAULT_API_HOST}{API_PATH}")
                    .parse()
                    .expect("invalid_smallestai_ws_url"),
                Vec::new(),
            )
        };

        if api_base.is_empty() {
            return default_url();
        }

        if let Some(proxy_result) = super::build_proxy_ws_url(api_base) {
            return proxy_result;
        }

        let parsed: url::Url = match api_base.parse() {
            Ok(url) => url,
            Err(_) => return default_url(),
        };

        let existing_params = super::extract_query_params(&parsed);
        (
            super::build_url_with_scheme(&parsed, DEFAULT_API_HOST, API_PATH, true),
            existing_params,
        )
    }

    pub(crate) fn batch_api_url(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        let default_url = || {
            (
                format!("https://{DEFAULT_API_HOST}{API_PATH}")
                    .parse()
                    .expect("invalid_smallestai_batch_url"),
                Vec::new(),
            )
        };

        if api_base.is_empty() {
            return default_url();
        }

        let parsed: url::Url = match api_base.parse() {
            Ok(url) => url,
            Err(_) => return default_url(),
        };

        let existing_params = super::extract_query_params(&parsed);
        (
            super::build_url_with_scheme(&parsed, DEFAULT_API_HOST, API_PATH, false),
            existing_params,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ws_url_from_base_empty() {
        let (url, params) = SmallestAIAdapter::build_ws_url_from_base("");
        assert_eq!(
            url.as_str(),
            "wss://api.smallest.ai/waves/v1/pulse/get_text"
        );
        assert!(params.is_empty());
    }

    #[test]
    fn test_build_ws_url_from_base_custom() {
        let (url, params) =
            SmallestAIAdapter::build_ws_url_from_base("https://stt.example.com/base?foo=bar");
        assert_eq!(
            url.as_str(),
            "wss://stt.example.com/waves/v1/pulse/get_text"
        );
        assert_eq!(params, vec![("foo".to_string(), "bar".to_string())]);
    }

    #[test]
    fn test_build_ws_url_from_base_proxy() {
        let (url, params) = SmallestAIAdapter::build_ws_url_from_base(
            "https://api.hyprnote.com/stt?provider=smallestai",
        );
        assert_eq!(url.as_str(), "wss://api.hyprnote.com/stt/listen");
        assert_eq!(
            params,
            vec![("provider".to_string(), "smallestai".to_string())]
        );
    }

    #[test]
    fn test_build_ws_url_from_base_local_proxy() {
        let (url, params) = SmallestAIAdapter::build_ws_url_from_base(
            "http://localhost:8787/stt?provider=smallestai",
        );
        assert_eq!(url.as_str(), "ws://localhost:8787/stt/listen");
        assert_eq!(
            params,
            vec![("provider".to_string(), "smallestai".to_string())]
        );
    }

    #[test]
    fn test_batch_api_url_empty() {
        let (url, params) = SmallestAIAdapter::batch_api_url("");
        assert_eq!(
            url.as_str(),
            "https://api.smallest.ai/waves/v1/pulse/get_text"
        );
        assert!(params.is_empty());
    }

    #[test]
    fn test_batch_api_url_custom() {
        let (url, params) =
            SmallestAIAdapter::batch_api_url("https://stt.example.com/base?foo=bar");
        assert_eq!(
            url.as_str(),
            "https://stt.example.com/waves/v1/pulse/get_text"
        );
        assert_eq!(params, vec![("foo".to_string(), "bar".to_string())]);
    }

    #[test]
    fn test_language_query_value() {
        use hypr_language::ISO639;

        assert_eq!(
            SmallestAIAdapter::language_query_value(&[ISO639::En.into()]),
            "en"
        );
        assert_eq!(
            SmallestAIAdapter::language_query_value(&[ISO639::En.into(), ISO639::Fr.into()]),
            "multi"
        );
        assert_eq!(SmallestAIAdapter::language_query_value(&[]), "multi");
    }

    #[test]
    fn test_language_support_rejects_unsupported_model() {
        use hypr_language::ISO639;

        assert_eq!(
            SmallestAIAdapter::language_support_live(&[ISO639::En.into()], Some("other-model")),
            LanguageSupport::NotSupported
        );
    }

    #[test]
    fn test_language_support_uses_documented_languages() {
        use hypr_language::ISO639;

        assert!(SmallestAIAdapter::is_supported_languages_live(
            &[ISO639::En.into(), ISO639::Fr.into()],
            Some("pulse"),
        ));
        assert!(!SmallestAIAdapter::is_supported_languages_live(
            &[ISO639::Ja.into()],
            Some("pulse"),
        ));
    }
}

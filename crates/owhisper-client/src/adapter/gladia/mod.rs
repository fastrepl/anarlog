mod batch;
mod language;
mod live;

use crate::providers::Provider;

use super::LanguageSupport;

#[derive(Clone, Default)]
pub struct GladiaAdapter;

impl GladiaAdapter {
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
        super::build_ws_url_from_base_with(Provider::Gladia, api_base, |parsed| {
            super::build_url_with_scheme(
                parsed,
                Provider::Gladia.default_api_host(),
                Provider::Gladia.ws_path(),
                true,
            )
        })
    }

    pub(crate) fn build_http_url(api_base: &str) -> url::Url {
        if api_base.is_empty() {
            return Self::default_http_url();
        }

        let parsed: url::Url = api_base.parse().expect("invalid_api_base");
        super::build_url_with_scheme(
            &parsed,
            Provider::Gladia.default_api_host(),
            Provider::Gladia.ws_path(),
            false,
        )
    }

    fn default_http_url() -> url::Url {
        format!(
            "https://{}{}",
            Provider::Gladia.default_api_host(),
            Provider::Gladia.ws_path()
        )
        .parse()
        .expect("invalid_default_http_url")
    }

    pub(crate) fn batch_api_url(api_base: &str) -> url::Url {
        if api_base.is_empty() {
            return "https://api.gladia.io/v2"
                .parse()
                .expect("invalid_default_api_url");
        }

        api_base.parse().expect("invalid_api_base")
    }
}

pub(super) fn documented_language_codes() -> &'static [&'static str] {
    language::SUPPORTED_LANGUAGES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ws_url_from_base() {
        let cases = [
            ("", "wss://api.gladia.io/v2/live", vec![]),
            (
                "https://api.gladia.io",
                "wss://api.gladia.io/v2/live",
                vec![],
            ),
            (
                "https://api.gladia.io:8443",
                "wss://api.gladia.io:8443/v2/live",
                vec![],
            ),
            (
                "https://api.hyprnote.com?provider=gladia",
                "wss://api.hyprnote.com/listen",
                vec![("provider", "gladia")],
            ),
            (
                "http://localhost:8787/listen?provider=gladia",
                "ws://localhost:8787/listen",
                vec![("provider", "gladia")],
            ),
        ];

        for (input, expected_url, expected_params) in cases {
            let (url, params) = GladiaAdapter::build_ws_url_from_base(input);
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
        assert!(Provider::Gladia.matches_url("https://api.gladia.io"));
        assert!(Provider::Gladia.matches_url("https://api.gladia.io/v2"));
        assert!(!Provider::Gladia.matches_url("https://api.deepgram.com"));
        assert!(!Provider::Gladia.matches_url("https://api.assemblyai.com"));
    }

    #[test]
    fn test_batch_api_url_empty_uses_default() {
        let url = GladiaAdapter::batch_api_url("");
        assert_eq!(url.as_str(), "https://api.gladia.io/v2");
    }

    #[test]
    fn test_batch_api_url_custom() {
        let url = GladiaAdapter::batch_api_url("https://custom.gladia.io/v2");
        assert_eq!(url.as_str(), "https://custom.gladia.io/v2");
    }
}

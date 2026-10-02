mod batch;
mod language;
mod live;

use super::{LanguageQuality, LanguageSupport};
use crate::providers::Provider;
use language::Mode;

pub(crate) const LIVE_PATH: &str = "/waves/v1/stt/live";
const BATCH_PATH: &str = "/waves/v1/stt/";
pub(crate) const DEFAULT_MODEL: &str = "pulse";
const PRO_MODEL: &str = "pulse-pro";
const MAX_KEYWORDS: usize = 100;

pub fn documented_language_codes_live() -> &'static [&'static str] {
    language::LIVE_LANGUAGES
}

pub fn documented_language_codes_batch() -> &'static [&'static str] {
    language::BATCH_LANGUAGES
}

#[derive(Clone, Default)]
pub struct SmallestAIAdapter;

impl SmallestAIAdapter {
    fn resolve_model(model: Option<&str>) -> Option<&'static str> {
        match model {
            None => Some(DEFAULT_MODEL),
            Some(model) if crate::providers::is_meta_model(model) => Some(DEFAULT_MODEL),
            Some(model) if model.eq_ignore_ascii_case(DEFAULT_MODEL) => Some(DEFAULT_MODEL),
            Some(model) if model.eq_ignore_ascii_case(PRO_MODEL) => Some(PRO_MODEL),
            Some(_) => None,
        }
    }

    pub(crate) fn batch_model(model: Option<&str>) -> &'static str {
        Self::resolve_model(model).unwrap_or(DEFAULT_MODEL)
    }

    fn language_support_impl(
        mode: Mode,
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        let Some(model) = Self::resolve_model(model) else {
            return LanguageSupport::NotSupported;
        };

        let supported = if model == PRO_MODEL {
            // Pulse Pro is English-only and has no streaming endpoint.
            mode == Mode::Batch
                && languages
                    .iter()
                    .all(|language| language.iso639().code() == "en")
        } else {
            language::query_value(mode, languages).is_some()
        };

        if supported {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        } else {
            LanguageSupport::NotSupported
        }
    }

    pub fn language_support_live(
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        Self::language_support_impl(Mode::Live, languages, model)
    }

    pub fn language_support_batch(
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        Self::language_support_impl(Mode::Batch, languages, model)
    }

    pub fn is_supported_languages_live(
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> bool {
        Self::language_support_live(languages, model).is_supported()
    }

    pub fn is_supported_languages_batch(
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> bool {
        Self::language_support_batch(languages, model).is_supported()
    }

    pub(crate) fn live_language_query_value(languages: &[anlg_language::Language]) -> String {
        language::query_value(Mode::Live, languages).unwrap_or_else(|| "en".to_string())
    }

    pub(crate) fn batch_language_query_value(
        languages: &[anlg_language::Language],
        model: &str,
    ) -> String {
        if model == PRO_MODEL {
            return "en".to_string();
        }
        language::query_value(Mode::Batch, languages).unwrap_or_else(|| "en".to_string())
    }

    /// Pulse takes `word` or `word:weight` entries in one comma-separated
    /// value, so terms containing either delimiter cannot be expressed.
    pub(crate) fn keywords_query_value(keywords: &[String]) -> Option<String> {
        let keywords: Vec<&str> = keywords
            .iter()
            .map(|keyword| keyword.trim())
            .filter(|keyword| !keyword.is_empty() && !keyword.contains([',', ':']))
            .take(MAX_KEYWORDS)
            .collect();

        (!keywords.is_empty()).then(|| keywords.join(","))
    }

    pub(crate) fn build_ws_url_from_base(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        super::build_ws_url_from_base_with(Provider::SmallestAI, api_base, |parsed| {
            super::build_url_with_scheme(
                parsed,
                Provider::SmallestAI.default_api_host(),
                LIVE_PATH,
                true,
            )
        })
    }

    pub(crate) fn batch_api_url(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        let default_host = Provider::SmallestAI.default_api_host();
        let default_url = || {
            (
                format!("https://{default_host}{BATCH_PATH}")
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
            super::build_url_with_scheme(&parsed, default_host, BATCH_PATH, false),
            existing_params,
        )
    }
}

#[cfg(test)]
mod tests {
    use anlg_language::ISO639;

    use super::*;

    #[test]
    fn test_build_ws_url_from_base() {
        for (input, expected_url, expected_params) in [
            ("", "wss://api.smallest.ai/waves/v1/stt/live", vec![]),
            (
                "https://api.us.smallest.ai/base?foo=bar",
                "wss://api.us.smallest.ai/waves/v1/stt/live",
                vec![("foo", "bar")],
            ),
            (
                "https://api.anarlog.so/stt?provider=smallestai",
                "wss://api.anarlog.so/stt/listen",
                vec![("provider", "smallestai")],
            ),
            (
                "http://localhost:8787/stt?provider=smallestai",
                "ws://localhost:8787/stt/listen",
                vec![("provider", "smallestai")],
            ),
        ] {
            let (url, params) = SmallestAIAdapter::build_ws_url_from_base(input);
            assert_eq!(url.as_str(), expected_url, "input: {input}");
            assert_eq!(
                params,
                expected_params
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<Vec<_>>(),
                "input: {input}"
            );
        }
    }

    #[test]
    fn test_batch_api_url() {
        for (input, expected_url, expected_params) in [
            ("", "https://api.smallest.ai/waves/v1/stt/", vec![]),
            (
                "https://api.us.smallest.ai/base?foo=bar",
                "https://api.us.smallest.ai/waves/v1/stt/",
                vec![("foo", "bar")],
            ),
        ] {
            let (url, params) = SmallestAIAdapter::batch_api_url(input);
            assert_eq!(url.as_str(), expected_url, "input: {input}");
            assert_eq!(
                params,
                expected_params
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<Vec<_>>(),
                "input: {input}"
            );
        }
    }

    #[test]
    fn test_batch_model_resolution() {
        assert_eq!(SmallestAIAdapter::batch_model(None), "pulse");
        assert_eq!(SmallestAIAdapter::batch_model(Some("cloud")), "pulse");
        assert_eq!(SmallestAIAdapter::batch_model(Some("Pulse")), "pulse");
        assert_eq!(
            SmallestAIAdapter::batch_model(Some("pulse-pro")),
            "pulse-pro"
        );
        assert_eq!(SmallestAIAdapter::batch_model(Some("other")), "pulse");
    }

    #[test]
    fn test_keywords_query_value() {
        assert_eq!(SmallestAIAdapter::keywords_query_value(&[]), None);
        assert_eq!(
            SmallestAIAdapter::keywords_query_value(&[
                " Anarlog ".to_string(),
                "".to_string(),
                "a,b".to_string(),
                "c:d".to_string(),
                "Pulse:2".to_string(),
                "Jensen Huang".to_string(),
            ]),
            Some("Anarlog,Jensen Huang".to_string())
        );
    }

    #[test]
    fn test_language_support() {
        assert_eq!(
            SmallestAIAdapter::language_support_live(&[ISO639::En.into()], Some("other-model")),
            LanguageSupport::NotSupported
        );

        assert!(SmallestAIAdapter::is_supported_languages_live(
            &[ISO639::En.into(), ISO639::Hi.into()],
            Some("pulse"),
        ));
        assert!(!SmallestAIAdapter::is_supported_languages_live(
            &[ISO639::Pl.into()],
            Some("pulse"),
        ));
        assert!(SmallestAIAdapter::is_supported_languages_batch(
            &[ISO639::Pl.into()],
            Some("pulse"),
        ));
        assert!(!SmallestAIAdapter::is_supported_languages_batch(
            &[ISO639::Gu.into()],
            Some("pulse"),
        ));

        assert!(!SmallestAIAdapter::is_supported_languages_live(
            &[ISO639::En.into()],
            Some("pulse-pro"),
        ));
        assert!(SmallestAIAdapter::is_supported_languages_batch(
            &[ISO639::En.into()],
            Some("pulse-pro"),
        ));
        assert!(SmallestAIAdapter::is_supported_languages_batch(
            &[],
            Some("pulse-pro"),
        ));
        assert!(!SmallestAIAdapter::is_supported_languages_batch(
            &[ISO639::En.into(), ISO639::Hi.into()],
            Some("pulse-pro"),
        ));
    }
}

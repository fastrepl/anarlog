mod batch;
mod live;

use crate::providers::Provider;

use super::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct FireworksAdapter;

impl FireworksAdapter {
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

    pub(crate) fn api_host(api_base: &str) -> String {
        if api_base.is_empty() {
            return Provider::Fireworks.default_api_host().to_string();
        }

        let url: url::Url = match api_base.parse() {
            Ok(u) => u,
            Err(_) => return Provider::Fireworks.default_api_host().to_string(),
        };
        url.host_str()
            .unwrap_or(Provider::Fireworks.default_api_host())
            .to_string()
    }

    pub(crate) fn batch_api_host(api_base: &str) -> String {
        let host = Self::api_host(api_base);
        format!("audio-turbo.{}", host)
    }

    pub(crate) fn ws_host(api_base: &str) -> String {
        let host = Self::api_host(api_base);
        format!("audio-streaming-v2.{}", host)
    }

    pub(crate) fn build_ws_url_from_base(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        super::build_ws_url_from_base_with(Provider::Fireworks, api_base, |_parsed| {
            format!(
                "wss://{}{}",
                Self::ws_host(api_base),
                Provider::Fireworks.ws_path()
            )
            .parse()
            .expect("invalid_ws_url")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ws_url_from_base() {
        let cases = [
            (
                "",
                "wss://audio-streaming-v2.api.fireworks.ai/v1/audio/transcriptions/streaming",
                vec![],
            ),
            (
                "https://api.fireworks.ai",
                "wss://audio-streaming-v2.api.fireworks.ai/v1/audio/transcriptions/streaming",
                vec![],
            ),
            (
                "https://api.anarlog.so/listen?provider=fireworks",
                "wss://api.anarlog.so/listen",
                vec![("provider", "fireworks")],
            ),
            (
                "http://localhost:8787/listen?provider=fireworks",
                "ws://localhost:8787/listen",
                vec![("provider", "fireworks")],
            ),
        ];

        for (input, expected_url, expected_params) in cases {
            let (url, params) = FireworksAdapter::build_ws_url_from_base(input);
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
}

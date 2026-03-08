mod batch;
mod live;

use crate::providers::Provider;

use super::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct FireworksAdapter;

impl FireworksAdapter {
    pub fn language_support_live(_languages: &[hypr_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }

    pub fn language_support_batch(_languages: &[hypr_language::Language]) -> LanguageSupport {
        Self::language_support_live(_languages)
    }

    pub fn is_supported_languages_live(languages: &[hypr_language::Language]) -> bool {
        Self::language_support_live(languages).is_supported()
    }

    pub fn is_supported_languages_batch(languages: &[hypr_language::Language]) -> bool {
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
    fn test_build_ws_url_from_base_empty() {
        let (url, params) = FireworksAdapter::build_ws_url_from_base("");
        assert_eq!(
            url.as_str(),
            "wss://audio-streaming-v2.api.fireworks.ai/v1/audio/transcriptions/streaming"
        );
        assert!(params.is_empty());
    }

    #[test]
    fn test_build_ws_url_from_base_fireworks() {
        let (url, params) = FireworksAdapter::build_ws_url_from_base("https://api.fireworks.ai");
        assert_eq!(
            url.as_str(),
            "wss://audio-streaming-v2.api.fireworks.ai/v1/audio/transcriptions/streaming"
        );
        assert!(params.is_empty());
    }

    #[test]
    fn test_build_ws_url_from_base_proxy() {
        let (url, params) = FireworksAdapter::build_ws_url_from_base(
            "https://api.hyprnote.com/listen?provider=fireworks",
        );
        assert_eq!(url.as_str(), "wss://api.hyprnote.com/listen");
        assert_eq!(params, vec![("provider".into(), "fireworks".into())]);
    }

    #[test]
    fn test_build_ws_url_from_base_localhost() {
        let (url, params) = FireworksAdapter::build_ws_url_from_base(
            "http://localhost:8787/listen?provider=fireworks",
        );
        assert_eq!(url.as_str(), "ws://localhost:8787/listen");
        assert_eq!(params, vec![("provider".into(), "fireworks".into())]);
    }
}

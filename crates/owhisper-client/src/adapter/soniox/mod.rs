mod batch;
mod callback;
pub mod error;
mod language;
mod live;

use super::LanguageSupport;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, strum::EnumString, strum::AsRefStr)]
pub enum SonioxModel {
    #[default]
    #[strum(
        serialize = "stt-v4",
        serialize = "stt-rt-v4",
        serialize = "stt-async-v4"
    )]
    V4,
    #[strum(
        serialize = "stt-v3",
        serialize = "stt-rt-v3",
        serialize = "stt-async-v3",
        serialize = "stt-rt-v3-preview",
        serialize = "stt-rt-preview-v2",
        serialize = "stt-async-preview-v1",
        serialize = "stt-async-preview"
    )]
    V3,
}

impl SonioxModel {
    pub fn live_model(&self) -> &'static str {
        match self {
            Self::V4 => "stt-rt-v4",
            Self::V3 => "stt-rt-v3",
        }
    }

    pub fn batch_model(&self) -> &'static str {
        match self {
            Self::V4 => "stt-async-v4",
            Self::V3 => "stt-async-v3",
        }
    }
}

#[derive(Clone, Default)]
pub struct SonioxAdapter;

impl SonioxAdapter {
    pub fn resolve_model(model: Option<&str>) -> SonioxModel {
        match model {
            Some(m) if crate::providers::is_meta_model(m) => SonioxModel::default(),
            Some(m) => m.parse::<SonioxModel>().unwrap_or_default(),
            None => SonioxModel::default(),
        }
    }

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

    pub(crate) fn api_host(api_base: &str) -> String {
        use crate::providers::Provider;

        let default_host = Provider::Soniox.default_api_host();

        if api_base.is_empty() {
            return default_host.to_string();
        }

        let url: url::Url = api_base.parse().expect("invalid_api_base");
        url.host_str().unwrap_or(default_host).to_string()
    }

    pub(crate) fn ws_host(api_base: &str) -> String {
        use crate::providers::Provider;

        let api_host = Self::api_host(api_base);

        if let Some(rest) = api_host.strip_prefix("api.") {
            format!("stt-rt.{}", rest)
        } else {
            Provider::Soniox.default_ws_host().to_string()
        }
    }

    pub(crate) fn build_ws_url_from_base(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        use crate::providers::Provider;

        super::build_ws_url_from_base_with(Provider::Soniox, api_base, |_parsed| {
            format!(
                "wss://{}{}",
                Self::ws_host(api_base),
                Provider::Soniox.ws_path()
            )
            .parse()
            .expect("invalid_ws_url")
        })
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
            ("", "wss://stt-rt.soniox.com/transcribe-websocket", vec![]),
            (
                "https://api.soniox.com",
                "wss://stt-rt.soniox.com/transcribe-websocket",
                vec![],
            ),
            (
                "https://api.hyprnote.com?provider=soniox",
                "wss://api.hyprnote.com/listen",
                vec![("provider", "soniox")],
            ),
            (
                "https://api.hyprnote.com/listen?provider=soniox",
                "wss://api.hyprnote.com/listen",
                vec![("provider", "soniox")],
            ),
            (
                "http://localhost:8787/listen?provider=soniox",
                "ws://localhost:8787/listen",
                vec![("provider", "soniox")],
            ),
        ];

        for (input, expected_url, expected_params) in cases {
            let (url, params) = SonioxAdapter::build_ws_url_from_base(input);
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
}

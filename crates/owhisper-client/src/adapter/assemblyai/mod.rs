mod batch;
pub(crate) mod error;
mod language;
mod live;

use super::LanguageSupport;

#[derive(Clone, Default)]
pub struct AssemblyAIAdapter;

impl AssemblyAIAdapter {
    pub fn language_support_live(languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::min(languages.iter().map(language::single_language_support_live))
    }

    pub fn language_support_batch(languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::min(
            languages
                .iter()
                .map(language::single_language_support_batch),
        )
    }

    pub fn is_supported_languages_live(languages: &[anlg_language::Language]) -> bool {
        Self::language_support_live(languages).is_supported()
    }

    pub fn is_supported_languages_batch(languages: &[anlg_language::Language]) -> bool {
        Self::language_support_batch(languages).is_supported()
    }
}

pub(super) fn documented_language_codes_live() -> &'static [&'static str] {
    language::STREAMING_LANGUAGES
}

pub(super) fn documented_language_codes_batch() -> &'static [&'static str] {
    language::BATCH_LANGUAGES
}

impl AssemblyAIAdapter {
    pub(crate) fn streaming_ws_url(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        use crate::providers::Provider;

        if api_base.is_empty() {
            return (
                Provider::AssemblyAI
                    .default_ws_url()
                    .parse()
                    .expect("invalid_default_ws_url"),
                Vec::new(),
            );
        }

        if let Some(proxy_result) = super::build_proxy_ws_url(api_base) {
            return proxy_result;
        }

        if api_base.contains(".eu.") || api_base.ends_with("-eu") {
            return (
                "wss://streaming.eu.assemblyai.com/v3/ws"
                    .parse()
                    .expect("invalid_eu_ws_url"),
                Vec::new(),
            );
        }

        let mut url: url::Url = api_base.parse().expect("invalid_api_base");
        let existing_params = super::extract_query_params(&url);
        url.set_query(None);

        if url.host_str() == Some("api.assemblyai.com") {
            let _ = url.set_host(Some("streaming.assemblyai.com"));
            url.set_path(Provider::AssemblyAI.ws_path());
        } else {
            super::append_path_if_missing(&mut url, Provider::AssemblyAI.ws_path());
        }

        super::set_scheme_from_host(&mut url);

        (url, existing_params)
    }

    pub(crate) fn batch_api_url(api_base: &str) -> url::Url {
        use crate::providers::Provider;

        if api_base.is_empty() {
            return Provider::AssemblyAI
                .default_api_url()
                .unwrap()
                .parse()
                .expect("invalid_default_api_url");
        }

        let mut url: url::Url = api_base.parse().expect("invalid_api_base");
        super::append_path_if_missing(&mut url, "v2");
        url
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Provider;

    #[test]
    fn streaming_ws_url_resolves_per_base() {
        let cases = [
            (
                "https://api.assemblyai.com",
                "wss://streaming.assemblyai.com/v3/ws",
                vec![],
            ),
            (
                Provider::AssemblyAI.default_api_base(),
                "wss://streaming.assemblyai.com/v3/ws",
                vec![],
            ),
            ("", "wss://streaming.assemblyai.com/v3/ws", vec![]),
            (
                "https://api.anarlog.so?provider=assemblyai",
                "wss://api.anarlog.so/listen",
                vec![("provider", "assemblyai")],
            ),
            (
                "http://localhost:8787?provider=assemblyai",
                "ws://localhost:8787/listen",
                vec![("provider", "assemblyai")],
            ),
        ];

        for (input, expected_url, expected_params) in cases {
            let (url, params) = AssemblyAIAdapter::streaming_ws_url(input);
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
    fn batch_api_url_resolves_per_base() {
        for input in [
            "",
            "https://api.assemblyai.com",
            "https://api.assemblyai.com/v2",
        ] {
            let url = AssemblyAIAdapter::batch_api_url(input);
            assert_eq!(
                url.as_str(),
                "https://api.assemblyai.com/v2",
                "input: {input}"
            );
        }
    }
}

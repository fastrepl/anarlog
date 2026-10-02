mod live;
mod streaming;

pub use streaming::DashScopeStreamingAdapter;

use crate::providers::Provider;

use super::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct DashScopeAdapter;

impl DashScopeAdapter {
    pub fn language_support_live(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }

    pub fn language_support_batch(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::NotSupported
    }

    pub fn is_supported_languages_live(languages: &[anlg_language::Language]) -> bool {
        Self::language_support_live(languages).is_supported()
    }

    pub fn is_supported_languages_batch(languages: &[anlg_language::Language]) -> bool {
        Self::language_support_batch(languages).is_supported()
    }

    pub(crate) fn build_ws_url_from_base(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        super::build_ws_url_from_base_with(Provider::DashScope, api_base, |parsed| {
            let host = parsed
                .host_str()
                .unwrap_or(Provider::DashScope.default_ws_host());
            let mut url: url::Url = format!("wss://{}{}", host, Provider::DashScope.ws_path())
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
    fn test_build_ws_url_from_base() {
        let cases = [
            (
                "",
                "wss://dashscope-intl.aliyuncs.com/api-ws/v1/realtime",
                vec![],
            ),
            (
                "wss://dashscope-intl.aliyuncs.com",
                "wss://dashscope-intl.aliyuncs.com/api-ws/v1/realtime",
                vec![],
            ),
            (
                "wss://dashscope.aliyuncs.com",
                "wss://dashscope.aliyuncs.com/api-ws/v1/realtime",
                vec![],
            ),
            (
                "https://api.anarlog.so?provider=dashscope",
                "wss://api.anarlog.so/listen",
                vec![("provider", "dashscope")],
            ),
            (
                "http://localhost:8787?provider=dashscope",
                "ws://localhost:8787/listen",
                vec![("provider", "dashscope")],
            ),
        ];

        for (input, expected_url, expected_params) in cases {
            let (url, params) = DashScopeAdapter::build_ws_url_from_base(input);
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
    fn test_is_dashscope_host() {
        assert!(Provider::DashScope.is_host("dashscope-intl.aliyuncs.com"));
        assert!(Provider::DashScope.is_host("dashscope.aliyuncs.com"));
        assert!(Provider::DashScope.is_host("aliyuncs.com"));
        assert!(!Provider::DashScope.is_host("api.openai.com"));
    }
}

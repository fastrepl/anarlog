mod live;

use crate::providers::Provider;

use super::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct DashScopeAdapter;

impl DashScopeAdapter {
    pub fn language_support_live(_languages: &[hypr_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }

    pub fn language_support_batch(_languages: &[hypr_language::Language]) -> LanguageSupport {
        LanguageSupport::NotSupported
    }

    pub fn is_supported_languages_live(languages: &[hypr_language::Language]) -> bool {
        Self::language_support_live(languages).is_supported()
    }

    pub fn is_supported_languages_batch(languages: &[hypr_language::Language]) -> bool {
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
    fn test_build_ws_url_from_base_empty() {
        let (url, params) = DashScopeAdapter::build_ws_url_from_base("");
        assert_eq!(
            url.as_str(),
            "wss://dashscope-intl.aliyuncs.com/api-ws/v1/realtime"
        );
        assert!(params.is_empty());
    }

    #[test]
    fn test_build_ws_url_from_base_intl() {
        let (url, params) =
            DashScopeAdapter::build_ws_url_from_base("wss://dashscope-intl.aliyuncs.com");
        assert_eq!(
            url.as_str(),
            "wss://dashscope-intl.aliyuncs.com/api-ws/v1/realtime"
        );
        assert!(params.is_empty());
    }

    #[test]
    fn test_build_ws_url_from_base_china() {
        let (url, params) =
            DashScopeAdapter::build_ws_url_from_base("wss://dashscope.aliyuncs.com");
        assert_eq!(
            url.as_str(),
            "wss://dashscope.aliyuncs.com/api-ws/v1/realtime"
        );
        assert!(params.is_empty());
    }

    #[test]
    fn test_build_ws_url_from_base_proxy() {
        let (url, params) =
            DashScopeAdapter::build_ws_url_from_base("https://api.hyprnote.com?provider=dashscope");
        assert_eq!(url.as_str(), "wss://api.hyprnote.com/listen");
        assert_eq!(
            params,
            vec![("provider".to_string(), "dashscope".to_string())]
        );
    }

    #[test]
    fn test_build_ws_url_from_base_localhost() {
        let (url, params) =
            DashScopeAdapter::build_ws_url_from_base("http://localhost:8787?provider=dashscope");
        assert_eq!(url.as_str(), "ws://localhost:8787/listen");
        assert_eq!(
            params,
            vec![("provider".to_string(), "dashscope".to_string())]
        );
    }

    #[test]
    fn test_is_dashscope_host() {
        assert!(Provider::DashScope.is_host("dashscope-intl.aliyuncs.com"));
        assert!(Provider::DashScope.is_host("dashscope.aliyuncs.com"));
        assert!(Provider::DashScope.is_host("aliyuncs.com"));
        assert!(!Provider::DashScope.is_host("api.openai.com"));
    }
}

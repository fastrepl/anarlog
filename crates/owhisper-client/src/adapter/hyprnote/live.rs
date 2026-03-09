use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::StreamResponse;

use super::HyprnoteAdapter;
use crate::adapter::{RealtimeSttAdapter, append_path_if_missing, set_scheme_from_host};

impl RealtimeSttAdapter for HyprnoteAdapter {
    fn provider_name(&self) -> &'static str {
        "hyprnote"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        HyprnoteAdapter::is_supported_languages_live(languages, model)
    }

    fn supports_native_multichannel(&self) -> bool {
        true
    }

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, channels: u8) -> url::Url {
        let mut url: url::Url = api_base.parse().expect("invalid api_base URL");

        set_scheme_from_host(&mut url);
        append_path_if_missing(&mut url, "listen");

        {
            let mut query = url.query_pairs_mut();

            if let Some(model) = &params.model {
                query.append_pair("model", model);
            }

            query.append_pair("channels", &channels.to_string());
            query.append_pair("sample_rate", &params.sample_rate.to_string());

            for lang in &params.languages {
                query.append_pair("language", lang.to_string().as_str());
            }

            for keyword in &params.keywords {
                query.append_pair("keyword", keyword);
            }

            if let Some(custom) = &params.custom_query {
                for (key, value) in custom {
                    query.append_pair(key, value);
                }
            }
        }

        url
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.map(|k| ("Authorization", format!("Bearer {}", k)))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        Some(Message::Text(
            serde_json::to_string(&owhisper_interface::ControlMessage::KeepAlive)
                .unwrap()
                .into(),
        ))
    }

    fn finalize_message(&self) -> Message {
        Message::Text(
            serde_json::to_string(&owhisper_interface::ControlMessage::Finalize)
                .unwrap()
                .into(),
        )
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        serde_json::from_str(raw).into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use hypr_language::ISO639;

    use super::HyprnoteAdapter;
    use crate::adapter::RealtimeSttAdapter;
    use crate::test_utils::{UrlTestCase, run_url_test_cases};

    const API_BASE: &str = "https://api.hyprnote.com/stt";

    #[test]
    fn test_url_structure() {
        run_url_test_cases(
            &HyprnoteAdapter::default(),
            API_BASE,
            &[
                UrlTestCase {
                    name: "single_language",
                    model: Some("nova-3"),
                    languages: &[ISO639::En],
                    contains: &["hyprnote.com", "listen", "model=nova-3", "language=en"],
                    not_contains: &[],
                },
                UrlTestCase {
                    name: "multi_language",
                    model: Some("stt-v3"),
                    languages: &[ISO639::En, ISO639::Ko],
                    contains: &["language=en", "language=ko"],
                    not_contains: &[],
                },
            ],
        );
    }

    #[test]
    fn test_url_with_keywords() {
        let adapter = HyprnoteAdapter::default();
        let params = owhisper_interface::ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![ISO639::En.into()],
            keywords: vec!["Hyprnote".to_string(), "transcription".to_string()],
            ..Default::default()
        };

        let url = adapter.build_ws_url(API_BASE, &params, 1);
        let url_str = url.as_str();

        assert!(url_str.contains("keyword=Hyprnote"));
        assert!(url_str.contains("keyword=transcription"));
    }

    #[test]
    fn test_url_with_custom_query() {
        let adapter = HyprnoteAdapter::default();
        let params = owhisper_interface::ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![ISO639::En.into()],
            custom_query: Some(
                [("provider".to_string(), "deepgram".to_string())]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        };

        let url = adapter.build_ws_url(API_BASE, &params, 1);
        assert!(url.as_str().contains("provider=deepgram"));
    }

    #[test]
    fn test_auth_header() {
        let adapter = HyprnoteAdapter::default();
        let header = adapter.build_auth_header(Some("test-key"));
        assert_eq!(
            header,
            Some(("Authorization", "Bearer test-key".to_string()))
        );
    }

    #[test]
    fn test_localhost_uses_ws_scheme() {
        let adapter = HyprnoteAdapter::default();
        let params = owhisper_interface::ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![ISO639::En.into()],
            ..Default::default()
        };

        let url = adapter.build_ws_url("http://localhost:8787/stt", &params, 1);
        assert!(url.scheme() == "ws");

        let url = adapter.build_ws_url("https://api.hyprnote.com/stt", &params, 1);
        assert!(url.scheme() == "wss");
    }

    #[test]
    fn test_meta_model_passed_through_without_resolution() {
        let adapter = HyprnoteAdapter::default();
        let params = owhisper_interface::ListenParams {
            model: Some("cloud".to_string()),
            languages: vec![ISO639::En.into(), ISO639::De.into()],
            ..Default::default()
        };

        let url = adapter.build_ws_url(API_BASE, &params, 1);
        let url_str = url.as_str();

        assert!(
            url_str.contains("model=cloud"),
            "meta-model 'cloud' should be passed through to proxy as-is, not resolved to a provider-specific model"
        );
        assert!(url_str.contains("language=en"));
        assert!(url_str.contains("language=de"));
    }

    #[test]
    fn test_provider_param_preserved_in_url() {
        let adapter = HyprnoteAdapter::default();
        let base_with_provider = "https://api.hyprnote.com/stt?provider=hyprnote";
        let params = owhisper_interface::ListenParams {
            model: Some("cloud".to_string()),
            languages: vec![ISO639::En.into()],
            ..Default::default()
        };

        let url = adapter.build_ws_url(base_with_provider, &params, 1);
        assert!(
            url.as_str().contains("provider=hyprnote"),
            "provider=hyprnote query param should be preserved in the final URL"
        );
    }
}

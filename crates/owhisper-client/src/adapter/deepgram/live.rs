use anlg_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::StreamResponse;

use crate::adapter::RealtimeSttAdapter;
use crate::adapter::deepgram_compat::build_listen_ws_url;

use super::{
    DeepgramAdapter, keywords::DeepgramKeywordStrategy, language::DeepgramLanguageStrategy,
};

impl RealtimeSttAdapter for DeepgramAdapter {
    fn provider_name(&self) -> &'static str {
        "deepgram"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> bool {
        if languages.is_empty() {
            return false;
        }
        DeepgramAdapter::is_supported_languages_live(languages, model)
    }

    fn supports_native_multichannel(&self) -> bool {
        true
    }

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, channels: u8) -> url::Url {
        build_listen_ws_url(
            api_base,
            params,
            channels,
            &DeepgramLanguageStrategy,
            &DeepgramKeywordStrategy,
        )
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|k| crate::providers::Provider::Deepgram.build_auth_header(k))
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
        let event = match serde_json::from_str(raw) {
            Ok(event) => event,
            Err(_error) => {
                tracing::warn!(
                    error.type = "invalid_provider_payload",
                    anarlog.payload.size_bytes = raw.len() as u64,
                    "deepgram_json_parse_failed"
                );
                return vec![];
            }
        };

        vec![event]
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use anlg_language::ISO639;

    use crate::adapter::RealtimeSttAdapter;
    use crate::test_utils::{UrlTestCase, run_url_test_cases};

    use super::DeepgramAdapter;

    const API_BASE: &str = "https://api.deepgram.com/v1";

    #[test]
    fn live_language_urls() {
        run_url_test_cases(
            &DeepgramAdapter::default(),
            API_BASE,
            &[
                UrlTestCase {
                    name: "english",
                    model: Some("nova-3"),
                    languages: &[ISO639::En],
                    contains: &["language=en"],
                    not_contains: &["language=multi", "languages=", "detect_language"],
                },
                UrlTestCase {
                    name: "japanese",
                    model: Some("nova-3"),
                    languages: &[ISO639::Ja],
                    contains: &["language=ja"],
                    not_contains: &["language=multi", "detect_language"],
                },
                UrlTestCase {
                    name: "empty_defaults_to_english",
                    model: Some("nova-3"),
                    languages: &[],
                    contains: &["language=en"],
                    not_contains: &["detect_language"],
                },
                UrlTestCase {
                    name: "nova3_en_es_supported",
                    model: Some("nova-3"),
                    languages: &[ISO639::En, ISO639::Es],
                    contains: &["language=multi"],
                    not_contains: &["languages=", "detect_language"],
                },
                UrlTestCase {
                    name: "nova3_en_fr_de_supported",
                    model: Some("nova-3"),
                    languages: &[ISO639::En, ISO639::Fr, ISO639::De],
                    contains: &["language=multi"],
                    not_contains: &["languages=", "detect_language"],
                },
                UrlTestCase {
                    name: "nova2_en_es_supported",
                    model: Some("nova-2"),
                    languages: &[ISO639::En, ISO639::Es],
                    contains: &["language=multi"],
                    not_contains: &["languages=", "detect_language"],
                },
                UrlTestCase {
                    name: "nova3_en_ko_unsupported",
                    model: Some("nova-3-general"),
                    languages: &[ISO639::En, ISO639::Ko],
                    contains: &["language=en"],
                    not_contains: &["language=multi", "languages=", "detect_language"],
                },
                UrlTestCase {
                    name: "nova2_en_fr_unsupported",
                    model: Some("nova-2"),
                    languages: &[ISO639::En, ISO639::Fr],
                    contains: &["language=en"],
                    not_contains: &["language=multi", "languages=", "detect_language"],
                },
            ],
        );
    }

    #[test]
    fn regional_variants_fall_back_or_are_preserved_per_model() {
        let adapter = DeepgramAdapter::default();

        let params = owhisper_interface::ListenParams {
            model: Some("nova-3-general".to_string()),
            languages: vec!["en-KR".parse().unwrap()],
            ..Default::default()
        };
        let url_str = adapter.build_ws_url(API_BASE, &params, 1);
        let url_str = url_str.as_str();
        assert!(url_str.contains("language=en"));
        assert!(!url_str.contains("language=en-KR"));

        let params = owhisper_interface::ListenParams {
            model: Some("nova-3-medical".to_string()),
            languages: vec!["en-CA".parse().unwrap()],
            ..Default::default()
        };
        let url_str = adapter.build_ws_url(API_BASE, &params, 1);
        assert!(url_str.as_str().contains("language=en-CA"));
    }

    #[test]
    fn live_url_carries_model_channels_custom_query_and_proxy_provider() {
        let adapter = DeepgramAdapter::default();
        let params = owhisper_interface::ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![ISO639::En.into()],
            custom_query: Some(HashMap::from([
                ("redemption_time_ms".to_string(), "400".to_string()),
                ("custom_param".to_string(), "test_value".to_string()),
            ])),
            ..Default::default()
        };

        let url = adapter.build_ws_url(API_BASE, &params, 1);
        let url_str = url.as_str();
        assert!(url_str.contains("model=nova-3"));
        assert!(url_str.contains("channels=1"));
        assert!(url_str.contains("redemption_time_ms=400"));
        assert!(url_str.contains("custom_param=test_value"));

        let params = owhisper_interface::ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![ISO639::En.into()],
            ..Default::default()
        };
        let url = adapter.build_ws_url(API_BASE, &params, 1);
        assert!(!url.as_str().contains("redemption_time_ms="));

        let url = adapter.build_ws_url("https://api.anarlog.so/stt?provider=deepgram", &params, 1);
        assert!(url.as_str().contains("provider=deepgram"));
    }
}

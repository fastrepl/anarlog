use hypr_ws_client::client::Message;
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
        languages: &[hypr_language::Language],
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
        serde_json::from_str(raw).into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use hypr_language::ISO639;

    use crate::ListenClient;
    use crate::adapter::RealtimeSttAdapter;
    use crate::test_utils::{UrlTestCase, run_dual_test, run_single_test, run_url_test_cases};

    use super::DeepgramAdapter;

    const API_BASE: &str = "https://api.deepgram.com/v1";

    #[test]
    fn test_single_language_urls() {
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
            ],
        );
    }

    #[test]
    fn test_multi_language_urls() {
        run_url_test_cases(
            &DeepgramAdapter::default(),
            API_BASE,
            &[
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
                    not_contains: &["languages="],
                },
                UrlTestCase {
                    name: "nova2_en_es_supported",
                    model: Some("nova-2"),
                    languages: &[ISO639::En, ISO639::Es],
                    contains: &["language=multi"],
                    not_contains: &["languages="],
                },
            ],
        );
    }

    #[test]
    fn test_unsupported_multi_language_fallback() {
        run_url_test_cases(
            &DeepgramAdapter::default(),
            API_BASE,
            &[
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
                    not_contains: &["language=multi", "languages="],
                },
            ],
        );
    }

    #[test]
    fn test_detect_language_never_in_live() {
        run_url_test_cases(
            &DeepgramAdapter::default(),
            API_BASE,
            &[
                UrlTestCase {
                    name: "empty_languages",
                    model: Some("nova-3"),
                    languages: &[],
                    contains: &[],
                    not_contains: &["detect_language"],
                },
                UrlTestCase {
                    name: "single_language",
                    model: Some("nova-3"),
                    languages: &[ISO639::En],
                    contains: &[],
                    not_contains: &["detect_language"],
                },
                UrlTestCase {
                    name: "unsupported_multi",
                    model: Some("nova-3"),
                    languages: &[ISO639::En, ISO639::Ko],
                    contains: &[],
                    not_contains: &["detect_language"],
                },
            ],
        );
    }

    #[test]
    fn test_invalid_regional_variant_falls_back_to_iso_language() {
        let adapter = DeepgramAdapter::default();
        let params = owhisper_interface::ListenParams {
            model: Some("nova-3-general".to_string()),
            languages: vec!["en-KR".parse().unwrap()],
            ..Default::default()
        };

        let url = adapter.build_ws_url(API_BASE, &params, 1);
        let url_str = url.as_str();

        assert!(url_str.contains("language=en"));
        assert!(!url_str.contains("language=en-KR"));
    }

    #[test]
    fn test_supported_regional_variant_is_preserved() {
        let adapter = DeepgramAdapter::default();
        let params = owhisper_interface::ListenParams {
            model: Some("nova-3-medical".to_string()),
            languages: vec!["en-CA".parse().unwrap()],
            ..Default::default()
        };

        let url = adapter.build_ws_url(API_BASE, &params, 1);
        let url_str = url.as_str();

        assert!(url_str.contains("language=en-CA"));
    }

    #[test]
    fn test_custom_query_params() {
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

        assert!(url_str.contains("redemption_time_ms=400"));
        assert!(url_str.contains("custom_param=test_value"));
    }

    #[test]
    fn test_proxy_preserves_provider_param() {
        let adapter = DeepgramAdapter::default();
        let params = owhisper_interface::ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![ISO639::En.into()],
            ..Default::default()
        };

        let url =
            adapter.build_ws_url("https://api.hyprnote.com/stt?provider=deepgram", &params, 1);

        assert!(url.as_str().contains("provider=deepgram"));
    }

    #[test]
    fn test_basic_url_params() {
        let adapter = DeepgramAdapter::default();
        let params = owhisper_interface::ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![ISO639::En.into()],
            ..Default::default()
        };

        let url = adapter.build_ws_url(API_BASE, &params, 1);
        let url_str = url.as_str();

        assert!(url_str.contains("model=nova-3"));
        assert!(url_str.contains("channels=1"));
        assert!(!url_str.contains("redemption_time_ms="));
    }

    macro_rules! single_test {
        ($name:ident, $params:expr) => {
            #[tokio::test]
            #[ignore]
            async fn $name() {
                let client = ListenClient::builder()
                    .api_base("https://api.deepgram.com/v1")
                    .api_key(std::env::var("DEEPGRAM_API_KEY").expect("DEEPGRAM_API_KEY not set"))
                    .params($params)
                    .build_single()
                    .await;
                run_single_test(client, "deepgram").await;
            }
        };
    }

    single_test!(
        test_single_with_keywords,
        owhisper_interface::ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![ISO639::En.into()],
            keywords: vec!["Hyprnote".to_string(), "transcription".to_string()],
            ..Default::default()
        }
    );

    single_test!(
        test_single_multi_lang_1,
        owhisper_interface::ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![ISO639::En.into(), ISO639::Es.into()],
            ..Default::default()
        }
    );

    single_test!(
        test_single_multi_lang_2,
        owhisper_interface::ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![ISO639::En.into(), ISO639::Ko.into()],
            ..Default::default()
        }
    );

    #[tokio::test]
    #[ignore]
    async fn test_build_dual() {
        let client = ListenClient::builder()
            .api_base("https://api.deepgram.com/v1")
            .api_key(std::env::var("DEEPGRAM_API_KEY").expect("DEEPGRAM_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                model: Some("nova-3".to_string()),
                languages: vec![ISO639::En.into()],
                ..Default::default()
            })
            .build_dual()
            .await;

        run_dual_test(client, "deepgram").await;
    }
}

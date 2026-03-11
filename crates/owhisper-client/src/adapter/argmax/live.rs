use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::StreamResponse;

use crate::adapter::RealtimeSttAdapter;
use crate::adapter::deepgram_compat::build_listen_ws_url;

use super::{ArgmaxAdapter, keywords::ArgmaxKeywordStrategy, language::ArgmaxLanguageStrategy};

impl RealtimeSttAdapter for ArgmaxAdapter {
    fn provider_name(&self) -> &'static str {
        "argmax"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        ArgmaxAdapter::is_supported_languages_live(languages, model)
    }

    fn supports_native_multichannel(&self) -> bool {
        false
    }

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, channels: u8) -> url::Url {
        build_listen_ws_url(
            api_base,
            params,
            channels,
            &ArgmaxLanguageStrategy,
            &ArgmaxKeywordStrategy,
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
        match serde_json::from_str::<StreamResponse>(raw) {
            Ok(response) => vec![response],
            Err(_) => {
                if let Ok(error) = serde_json::from_str::<ArgmaxError>(raw) {
                    tracing::error!(
                        error.type = %error.error_type,
                        error = %error.message,
                        "argmax_error"
                    );
                    vec![StreamResponse::ErrorResponse {
                        error_code: None,
                        error_message: format!("{}: {}", error.error_type, error.message),
                        provider: "argmax".to_string(),
                    }]
                } else {
                    tracing::warn!(
                        hyprnote.payload.size_bytes = raw.len() as u64,
                        "argmax_unknown_message"
                    );
                    vec![]
                }
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct ArgmaxError {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

#[cfg(test)]
mod tests {
    use hypr_language::ISO639;

    use super::ArgmaxAdapter;
    use crate::ListenClient;
    use crate::test_utils::{UrlTestCase, run_dual_test, run_single_test, run_url_test_cases};

    const API_BASE: &str = "ws://localhost:50060/v1";

    #[test]
    fn test_single_language_urls() {
        run_url_test_cases(
            &ArgmaxAdapter::default(),
            API_BASE,
            &[
                UrlTestCase {
                    name: "english",
                    model: None,
                    languages: &[ISO639::En],
                    contains: &["language=en", "detect_language=false"],
                    not_contains: &["language=multi", "languages="],
                },
                UrlTestCase {
                    name: "empty_defaults_to_english",
                    model: None,
                    languages: &[],
                    contains: &["language=en"],
                    not_contains: &["detect_language=false"],
                },
            ],
        );
    }

    #[test]
    fn test_multi_language_urls() {
        run_url_test_cases(
            &ArgmaxAdapter::default(),
            API_BASE,
            &[UrlTestCase {
                name: "multi_lang_picks_first",
                model: None,
                languages: &[ISO639::De, ISO639::Fr],
                contains: &["language=de", "detect_language=false"],
                not_contains: &["language=multi", "language=fr"],
            }],
        );
    }

    #[test]
    fn test_parakeet_v2_urls() {
        run_url_test_cases(
            &ArgmaxAdapter::default(),
            API_BASE,
            &[UrlTestCase {
                name: "parakeet_v2_always_english",
                model: Some("parakeet-v2-something"),
                languages: &[ISO639::De],
                contains: &["language=en", "detect_language=false"],
                not_contains: &["language=de"],
            }],
        );
    }

    #[test]
    fn test_parakeet_v3_urls() {
        run_url_test_cases(
            &ArgmaxAdapter::default(),
            API_BASE,
            &[
                UrlTestCase {
                    name: "parakeet_v3_supported_language",
                    model: Some("parakeet-v3-something"),
                    languages: &[ISO639::De],
                    contains: &["language=de", "detect_language=false"],
                    not_contains: &[],
                },
                UrlTestCase {
                    name: "parakeet_v3_unsupported_fallback",
                    model: Some("parakeet-v3-something"),
                    languages: &[ISO639::Ko],
                    contains: &["language=en", "detect_language=false"],
                    not_contains: &["language=ko"],
                },
                UrlTestCase {
                    name: "parakeet_v3_multi_lang_picks_first_supported",
                    model: Some("parakeet-v3-something"),
                    languages: &[ISO639::Ko, ISO639::Fr],
                    contains: &["language=fr", "detect_language=false"],
                    not_contains: &[],
                },
            ],
        );
    }

    macro_rules! single_test {
        ($name:ident, $params:expr) => {
            #[tokio::test]
            #[ignore]
            async fn $name() {
                let client = ListenClient::builder()
                    .adapter::<ArgmaxAdapter>()
                    .api_base("ws://localhost:50060/v1")
                    .api_key("")
                    .params($params)
                    .build_single()
                    .await;
                run_single_test(client, "argmax").await;
            }
        };
    }

    single_test!(
        test_build_single,
        owhisper_interface::ListenParams {
            model: Some("large-v3-v20240930_626MB".to_string()),
            languages: vec![hypr_language::ISO639::En.into()],
            ..Default::default()
        }
    );

    single_test!(
        test_single_with_keywords,
        owhisper_interface::ListenParams {
            model: Some("large-v3-v20240930_626MB".to_string()),
            languages: vec![hypr_language::ISO639::En.into()],
            keywords: vec!["Hyprnote".to_string(), "transcription".to_string()],
            ..Default::default()
        }
    );

    #[tokio::test]
    #[ignore]
    async fn test_build_dual() {
        let client = ListenClient::builder()
            .adapter::<ArgmaxAdapter>()
            .api_base("ws://localhost:50060/v1")
            .api_key("")
            .params(owhisper_interface::ListenParams {
                model: Some("large-v3-v20240930_626MB".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_dual()
            .await;

        run_dual_test(client, "argmax").await;
    }
}

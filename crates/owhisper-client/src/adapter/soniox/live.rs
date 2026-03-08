use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::Serialize;

use super::SonioxAdapter;
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::{WordBuilder, ms_to_secs_opt};

// https://soniox.com/docs/stt/rt/real-time-transcription
// https://soniox.com/docs/stt/api-reference/websocket-api
impl RealtimeSttAdapter for SonioxAdapter {
    fn provider_name(&self) -> &'static str {
        "soniox"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        SonioxAdapter::is_supported_languages_live(languages)
    }

    fn supports_native_multichannel(&self) -> bool {
        false
    }

    fn build_ws_url(&self, api_base: &str, _params: &ListenParams, _channels: u8) -> url::Url {
        let (mut url, existing_params) = Self::build_ws_url_from_base(api_base);

        if !existing_params.is_empty() {
            let mut query_pairs = url.query_pairs_mut();
            for (key, value) in &existing_params {
                query_pairs.append_pair(key, value);
            }
        }

        url
    }

    fn build_auth_header(&self, _api_key: Option<&str>) -> Option<(&'static str, String)> {
        None
    }

    // https://soniox.com/docs/stt/rt/connection-keepalive
    fn keep_alive_message(&self) -> Option<Message> {
        Some(Message::Text(r#"{"type":"keepalive"}"#.into()))
    }

    fn initial_message(
        &self,
        api_key: Option<&str>,
        params: &ListenParams,
        channels: u8,
    ) -> Option<Message> {
        let api_key = api_key.unwrap_or("");

        let model = SonioxAdapter::resolve_model(params.model.as_deref()).live_model();

        let context = if params.keywords.is_empty() {
            None
        } else {
            Some(Context {
                terms: params.keywords.clone(),
                ..Default::default()
            })
        };

        let language_hints: Vec<String> = params
            .languages
            .iter()
            .map(|lang| lang.iso639().code().to_string())
            .collect();

        let cfg = SonioxConfig {
            api_key,
            model,
            audio_format: "pcm_s16le",
            num_channels: channels,
            sample_rate: params.sample_rate,
            language_hints_strict: !language_hints.is_empty(),
            language_hints,
            enable_endpoint_detection: true,
            enable_speaker_diarization: true,
            context,
        };

        let json = serde_json::to_string(&cfg).unwrap();
        Some(Message::Text(json.into()))
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let msg: soniox::StreamMessage = match serde_json::from_str(raw) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    error.message = ?e,
                    hyprnote.payload.raw = raw,
                    "soniox_json_parse_failed"
                );
                return vec![];
            }
        };

        if let Some(error_msg) = &msg.error_message {
            tracing::error!(
                error.code = ?msg.error_code,
                error.message = %error_msg,
                "soniox_error"
            );
            return vec![StreamResponse::ErrorResponse {
                error_code: msg.error_code,
                error_message: error_msg.clone(),
                provider: "soniox".to_string(),
            }];
        }

        let has_fin_token = msg.tokens.iter().any(|t| t.is_fin());
        let is_finished =
            msg.finished.unwrap_or(false) || msg.tokens.iter().any(|t| t.is_fin() || t.is_end());

        let content_tokens: Vec<_> = msg.tokens.into_iter().filter(|t| !t.is_control()).collect();

        if content_tokens.is_empty() && !is_finished {
            return vec![];
        }

        let final_tokens: Vec<_> = content_tokens
            .iter()
            .filter(|t| t.is_final.unwrap_or(true))
            .collect();

        let non_final_tokens: Vec<_> = content_tokens
            .iter()
            .filter(|t| !t.is_final.unwrap_or(true))
            .collect();

        let mut responses = Vec::new();

        if !final_tokens.is_empty() {
            responses.push(Self::build_response(
                &final_tokens,
                true,
                is_finished,
                has_fin_token,
            ));
        }

        if !non_final_tokens.is_empty() {
            responses.push(Self::build_response(&non_final_tokens, false, false, false));
        }

        responses
    }

    // https://soniox.com/docs/stt/rt/manual-finalization
    fn finalize_message(&self) -> Message {
        Message::Text(r#"{"type":"finalize"}"#.into())
    }
}

#[derive(Default, Serialize)]
struct Context {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    general: Vec<ContextGeneral>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    terms: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    translation_terms: Vec<TranslationTerm>,
}

#[derive(Serialize)]
struct ContextGeneral {
    key: String,
    value: String,
}

#[derive(Serialize)]
struct TranslationTerm {
    source: String,
    target: String,
}

#[derive(Serialize)]
struct SonioxConfig<'a> {
    api_key: &'a str,
    model: &'a str,
    audio_format: &'a str,
    num_channels: u8,
    sample_rate: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    language_hints: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    language_hints_strict: bool,
    enable_endpoint_detection: bool,
    enable_speaker_diarization: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<Context>,
}

impl SonioxAdapter {
    fn build_response(
        tokens: &[&soniox::Token],
        is_final: bool,
        speech_final: bool,
        from_finalize: bool,
    ) -> StreamResponse {
        let mut words = Vec::with_capacity(tokens.len());
        let mut transcript = String::new();

        for t in tokens {
            if t.text.trim().is_empty() {
                transcript.push_str(&t.text);
                continue;
            }

            transcript.push_str(&t.text);

            let start_secs = ms_to_secs_opt(t.start_ms);
            let end_secs = ms_to_secs_opt(t.end_ms);
            let speaker = t.speaker.as_ref().and_then(|s| s.as_i32());

            words.push(
                WordBuilder::new(&t.text)
                    .start(start_secs)
                    .end(end_secs)
                    .confidence(t.confidence.unwrap_or(1.0))
                    .speaker(speaker)
                    .build(),
            );
        }

        let (start, duration) = if let (Some(first), Some(last)) = (tokens.first(), tokens.last()) {
            let start_secs = ms_to_secs_opt(first.start_ms);
            let end_secs = ms_to_secs_opt(last.end_ms);
            (start_secs, end_secs - start_secs)
        } else {
            (0.0, 0.0)
        };

        let channel = Channel {
            alternatives: vec![Alternatives {
                transcript,
                words,
                confidence: 1.0,
                languages: vec![],
            }],
        };

        StreamResponse::TranscriptResponse {
            is_final,
            speech_final,
            from_finalize,
            start,
            duration,
            channel,
            metadata: Metadata::default(),
            channel_index: vec![0, 1],
        }
    }
}

#[cfg(test)]
mod tests {
    use hypr_language::ISO639;
    use hypr_ws_client::client::Message;

    use super::SonioxAdapter;
    use crate::ListenClient;
    use crate::adapter::RealtimeSttAdapter;
    use crate::test_utils::{UrlTestCase, run_dual_test, run_single_test, run_url_test_cases};

    const API_BASE: &str = "https://api.soniox.com";

    #[test]
    fn test_base_url() {
        run_url_test_cases(
            &SonioxAdapter::default(),
            API_BASE,
            &[UrlTestCase {
                name: "base_url_structure",
                model: None,
                languages: &[ISO639::En],
                contains: &["soniox.com"],
                not_contains: &[],
            }],
        );
    }

    fn extract_initial_message_json(
        adapter: &SonioxAdapter,
        params: &owhisper_interface::ListenParams,
    ) -> serde_json::Value {
        let msg = adapter
            .initial_message(Some("test_key"), params, 1)
            .unwrap();
        match msg {
            Message::Text(text) => serde_json::from_str(&text).unwrap(),
            _ => panic!("Expected text message"),
        }
    }

    #[test]
    fn test_initial_message_single_language() {
        let adapter = SonioxAdapter::default();
        let params = owhisper_interface::ListenParams {
            languages: vec![hypr_language::ISO639::En.into()],
            ..Default::default()
        };

        let json = extract_initial_message_json(&adapter, &params);

        let hints = json["language_hints"].as_array().unwrap();
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].as_str().unwrap(), "en");
        assert_eq!(json["language_hints_strict"].as_bool().unwrap(), true);
    }

    #[test]
    fn test_initial_message_multi_language() {
        let adapter = SonioxAdapter::default();
        let params = owhisper_interface::ListenParams {
            languages: vec![
                hypr_language::ISO639::En.into(),
                hypr_language::ISO639::Ko.into(),
            ],
            ..Default::default()
        };

        let json = extract_initial_message_json(&adapter, &params);

        let hints = json["language_hints"].as_array().unwrap();
        assert_eq!(hints.len(), 2);
        assert_eq!(hints[0].as_str().unwrap(), "en");
        assert_eq!(hints[1].as_str().unwrap(), "ko");
        assert_eq!(json["language_hints_strict"].as_bool().unwrap(), true);
    }

    #[test]
    fn test_initial_message_empty_languages() {
        let adapter = SonioxAdapter::default();
        let params = owhisper_interface::ListenParams {
            languages: vec![],
            ..Default::default()
        };

        let json = extract_initial_message_json(&adapter, &params);

        assert!(
            json.get("language_hints").is_none()
                || json["language_hints"].as_array().unwrap().is_empty(),
            "Empty languages should result in no language_hints"
        );
        assert!(
            json.get("language_hints_strict").is_none()
                || !json["language_hints_strict"].as_bool().unwrap_or(false),
            "Empty languages should not have language_hints_strict=true"
        );
    }

    #[test]
    fn test_initial_message_three_languages() {
        let adapter = SonioxAdapter::default();
        let params = owhisper_interface::ListenParams {
            languages: vec![
                hypr_language::ISO639::En.into(),
                hypr_language::ISO639::Es.into(),
                hypr_language::ISO639::Fr.into(),
            ],
            ..Default::default()
        };

        let json = extract_initial_message_json(&adapter, &params);

        let hints = json["language_hints"].as_array().unwrap();
        assert_eq!(hints.len(), 3);
        assert_eq!(hints[0].as_str().unwrap(), "en");
        assert_eq!(hints[1].as_str().unwrap(), "es");
        assert_eq!(hints[2].as_str().unwrap(), "fr");
        assert_eq!(json["language_hints_strict"].as_bool().unwrap(), true);
    }

    macro_rules! single_test {
        ($name:ident, $params:expr) => {
            #[tokio::test]
            #[ignore]
            async fn $name() {
                let client = ListenClient::builder()
                    .adapter::<SonioxAdapter>()
                    .api_base("https://api.soniox.com")
                    .api_key(std::env::var("SONIOX_API_KEY").expect("SONIOX_API_KEY not set"))
                    .params($params)
                    .build_single()
                    .await;
                run_single_test(client, "soniox").await;
            }
        };
    }

    single_test!(
        test_build_single,
        owhisper_interface::ListenParams {
            model: Some("stt-v3".to_string()),
            languages: vec![hypr_language::ISO639::En.into()],
            ..Default::default()
        }
    );

    single_test!(
        test_single_with_keywords,
        owhisper_interface::ListenParams {
            model: Some("stt-v3".to_string()),
            languages: vec![hypr_language::ISO639::En.into()],
            keywords: vec!["Hyprnote".to_string(), "transcription".to_string()],
            ..Default::default()
        }
    );

    single_test!(
        test_single_multi_lang_1,
        owhisper_interface::ListenParams {
            model: Some("stt-v3".to_string()),
            languages: vec![
                hypr_language::ISO639::En.into(),
                hypr_language::ISO639::Es.into(),
            ],
            ..Default::default()
        }
    );

    single_test!(
        test_single_multi_lang_2,
        owhisper_interface::ListenParams {
            model: Some("stt-v3".to_string()),
            languages: vec![
                hypr_language::ISO639::En.into(),
                hypr_language::ISO639::Ko.into(),
            ],
            ..Default::default()
        }
    );

    #[tokio::test]
    #[ignore]
    async fn test_build_dual() {
        let client = ListenClient::builder()
            .adapter::<SonioxAdapter>()
            .api_base("https://api.soniox.com")
            .api_key(std::env::var("SONIOX_API_KEY").expect("SONIOX_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                model: Some("stt-v3".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_dual()
            .await;

        run_dual_test(client, "soniox").await;
    }
}

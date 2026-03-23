use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::Serialize;

use super::SonioxAdapter;
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::{WordBuilder, calculate_time_span, ms_to_secs_opt};

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
                    error = ?e,
                    hyprnote.payload.size_bytes = raw.len() as u64,
                    "soniox_json_parse_failed"
                );
                return vec![];
            }
        };

        if let Some(error_msg) = &msg.error_message {
            tracing::error!(
                error.code = ?msg.error_code,
                error = %error_msg,
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

        let (final_tokens, non_final_tokens) = partition_tokens_by_word_finality(&content_tokens);

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
        let transcript = tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect::<String>();
        let words = build_words(tokens);
        let (start, duration) = calculate_time_span(&words);

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

fn build_words(tokens: &[&soniox::Token]) -> Vec<owhisper_interface::stream::Word> {
    #[derive(Default)]
    struct PendingWord {
        text: String,
        start_ms: Option<u64>,
        end_ms: Option<u64>,
        speaker: Option<i32>,
        language: Option<String>,
        confidence_sum: f64,
        confidence_count: u32,
    }

    impl PendingWord {
        fn push_token(&mut self, token: &soniox::Token, text: &str) {
            if self.start_ms.is_none() {
                self.start_ms = token.start_ms;
            }
            self.end_ms = token.end_ms.or(self.end_ms);
            self.text.push_str(text);

            if self.speaker.is_none() {
                self.speaker = token.speaker.as_ref().and_then(|speaker| speaker.as_i32());
            }
            if self.language.is_none() {
                self.language = token.language.clone();
            }

            self.confidence_sum += token.confidence.unwrap_or(1.0);
            self.confidence_count += 1;
        }

        fn build(self) -> Option<owhisper_interface::stream::Word> {
            if self.text.is_empty() {
                return None;
            }

            let confidence = if self.confidence_count == 0 {
                1.0
            } else {
                self.confidence_sum / f64::from(self.confidence_count)
            };

            Some(
                WordBuilder::new(self.text)
                    .start(ms_to_secs_opt(self.start_ms))
                    .end(ms_to_secs_opt(self.end_ms))
                    .confidence(confidence)
                    .speaker(self.speaker)
                    .language(self.language)
                    .build(),
            )
        }
    }

    token_groups_from_refs(tokens)
        .into_iter()
        .filter_map(|group| {
            let mut pending = PendingWord::default();
            for token in group {
                let trimmed = token.text.trim();
                if !trimmed.is_empty() {
                    pending.push_token(token, trimmed);
                }
            }
            pending.build()
        })
        .collect()
}

fn partition_tokens_by_word_finality<'a>(
    tokens: &'a [soniox::Token],
) -> (Vec<&'a soniox::Token>, Vec<&'a soniox::Token>) {
    let mut final_tokens = Vec::new();
    let mut non_final_tokens = Vec::new();
    for group in token_groups_from_values(tokens) {
        if group.iter().all(|token| token.is_final.unwrap_or(true)) {
            final_tokens.extend(group);
        } else {
            non_final_tokens.extend(group);
        }
    }

    (final_tokens, non_final_tokens)
}

fn token_groups_from_refs<'a>(tokens: &[&'a soniox::Token]) -> Vec<Vec<&'a soniox::Token>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    let mut current_has_content = false;

    let flush = |groups: &mut Vec<Vec<&'a soniox::Token>>, current: &mut Vec<&'a soniox::Token>| {
        if !current.is_empty() {
            groups.push(std::mem::take(current));
        }
    };

    for token in tokens {
        let has_content = !token.text.trim().is_empty();
        let starts_with_ws = token.text.chars().next().is_some_and(char::is_whitespace);

        if starts_with_ws && current_has_content {
            flush(&mut groups, &mut current);
            current_has_content = false;
        }

        current.push(*token);
        current_has_content |= has_content;
    }

    flush(&mut groups, &mut current);
    groups
}

fn token_groups_from_values<'a>(tokens: &'a [soniox::Token]) -> Vec<Vec<&'a soniox::Token>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    let mut current_has_content = false;

    let flush = |groups: &mut Vec<Vec<&'a soniox::Token>>, current: &mut Vec<&'a soniox::Token>| {
        if !current.is_empty() {
            groups.push(std::mem::take(current));
        }
    };

    for token in tokens {
        let has_content = !token.text.trim().is_empty();
        let starts_with_ws = token.text.chars().next().is_some_and(char::is_whitespace);

        if starts_with_ws && current_has_content {
            flush(&mut groups, &mut current);
            current_has_content = false;
        }

        current.push(token);
        current_has_content |= has_content;
    }

    flush(&mut groups, &mut current);
    groups
}

#[cfg(test)]
mod tests {
    use hypr_language::ISO639;
    use hypr_ws_client::client::Message;
    use owhisper_interface::stream::StreamResponse;

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

    #[test]
    fn parse_response_keeps_split_word_partial_until_complete() {
        let responses = SonioxAdapter.parse_response(
            r#"{
                "tokens": [
                    { "text": " hundreds", "start_ms": 0, "end_ms": 100, "is_final": true },
                    { "text": " of", "start_ms": 100, "end_ms": 200, "is_final": true },
                    { "text": " mill", "start_ms": 200, "end_ms": 300, "is_final": true },
                    { "text": "ions.", "start_ms": 300, "end_ms": 450, "is_final": false }
                ]
            }"#,
        );

        assert_eq!(responses.len(), 2);

        let final_words = match &responses[0] {
            StreamResponse::TranscriptResponse {
                channel, is_final, ..
            } => {
                assert!(*is_final);
                &channel.alternatives[0].words
            }
            _ => panic!("expected transcript response"),
        };
        assert_eq!(final_words.len(), 2);
        assert_eq!(final_words[0].word, "hundreds");
        assert_eq!(final_words[1].word, "of");

        let partial_words = match &responses[1] {
            StreamResponse::TranscriptResponse {
                channel, is_final, ..
            } => {
                assert!(!*is_final);
                &channel.alternatives[0].words
            }
            _ => panic!("expected transcript response"),
        };
        assert_eq!(partial_words.len(), 1);
        assert_eq!(partial_words[0].word, "millions.");
    }

    #[test]
    fn parse_response_merges_subword_tokens_into_single_word() {
        let responses = SonioxAdapter.parse_response(
            r#"{
                "tokens": [
                    { "text": " mill", "start_ms": 0, "end_ms": 50, "is_final": true },
                    { "text": "ions", "start_ms": 50, "end_ms": 120, "is_final": true },
                    { "text": ".", "start_ms": 120, "end_ms": 150, "is_final": true },
                    { "text": "<end>", "is_final": true }
                ]
            }"#,
        );

        assert_eq!(responses.len(), 1);

        let words = match &responses[0] {
            StreamResponse::TranscriptResponse {
                channel,
                is_final,
                speech_final,
                ..
            } => {
                assert!(*is_final);
                assert!(*speech_final);
                &channel.alternatives[0].words
            }
            _ => panic!("expected transcript response"),
        };

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].word, "millions.");
        assert_eq!(words[0].start, 0.0);
        assert_eq!(words[0].end, 0.15);
    }

    #[test]
    fn parse_response_keeps_korean_eojeol_partial_when_finality_splits_mid_word() {
        let responses = SonioxAdapter.parse_response(
            r#"{
                "tokens": [
                    { "text": "입", "start_ms": 480, "end_ms": 540, "is_final": true, "speaker": "1" },
                    { "text": "니", "start_ms": 660, "end_ms": 720, "is_final": true, "speaker": "1" },
                    { "text": "다.", "start_ms": 720, "end_ms": 780, "is_final": true, "speaker": "1" },
                    { "text": " 단", "start_ms": 1440, "end_ms": 1500, "is_final": true, "speaker": "1" },
                    { "text": "서", "start_ms": 1620, "end_ms": 1680, "is_final": false, "speaker": "1" },
                    { "text": "는", "start_ms": 1740, "end_ms": 1800, "is_final": false, "speaker": "1" }
                ]
            }"#,
        );

        assert_eq!(responses.len(), 2);

        let final_alt = match &responses[0] {
            StreamResponse::TranscriptResponse {
                channel, is_final, ..
            } => {
                assert!(*is_final);
                &channel.alternatives[0]
            }
            _ => panic!("expected transcript response"),
        };
        assert_eq!(final_alt.transcript, "입니다.");
        assert_eq!(final_alt.words.len(), 1);
        assert_eq!(final_alt.words[0].word, "입니다.");

        let partial_alt = match &responses[1] {
            StreamResponse::TranscriptResponse {
                channel, is_final, ..
            } => {
                assert!(!*is_final);
                &channel.alternatives[0]
            }
            _ => panic!("expected transcript response"),
        };
        assert_eq!(partial_alt.transcript, " 단서는");
        assert_eq!(partial_alt.words.len(), 1);
        assert_eq!(partial_alt.words[0].word, "단서는");
    }

    #[test]
    fn parse_response_keeps_korean_partial_prefix_out_of_final_chunk() {
        let responses = SonioxAdapter.parse_response(
            r#"{
                "tokens": [
                    { "text": " 정치", "start_ms": 3720, "end_ms": 3780, "is_final": true, "speaker": "1" },
                    { "text": "적", "start_ms": 3960, "end_ms": 4020, "is_final": true, "speaker": "1" },
                    { "text": " 분", "start_ms": 4140, "end_ms": 4200, "is_final": true, "speaker": "1" },
                    { "text": "열", "start_ms": 4260, "end_ms": 4320, "is_final": true, "speaker": "1" },
                    { "text": " 속", "start_ms": 4380, "end_ms": 4440, "is_final": true, "speaker": "1" },
                    { "text": "에서", "start_ms": 4560, "end_ms": 4620, "is_final": true, "speaker": "1" },
                    { "text": " 매", "start_ms": 5100, "end_ms": 5160, "is_final": true, "speaker": "1" },
                    { "text": "우", "start_ms": 5160, "end_ms": 5220, "is_final": true, "speaker": "1" },
                    { "text": " 불", "start_ms": 5520, "end_ms": 5580, "is_final": true, "speaker": "1" },
                    { "text": "안", "start_ms": 5700, "end_ms": 5760, "is_final": false, "speaker": "1" },
                    { "text": "정", "start_ms": 5820, "end_ms": 5880, "is_final": false, "speaker": "1" },
                    { "text": "한", "start_ms": 6000, "end_ms": 6060, "is_final": false, "speaker": "1" }
                ]
            }"#,
        );

        assert_eq!(responses.len(), 2);

        let final_alt = match &responses[0] {
            StreamResponse::TranscriptResponse {
                channel, is_final, ..
            } => {
                assert!(*is_final);
                &channel.alternatives[0]
            }
            _ => panic!("expected transcript response"),
        };
        assert_eq!(final_alt.transcript, " 정치적 분열 속에서 매우");

        let partial_alt = match &responses[1] {
            StreamResponse::TranscriptResponse {
                channel, is_final, ..
            } => {
                assert!(!*is_final);
                &channel.alternatives[0]
            }
            _ => panic!("expected transcript response"),
        };
        assert_eq!(partial_alt.transcript, " 불안정한");
        assert_eq!(partial_alt.words.len(), 1);
        assert_eq!(partial_alt.words[0].word, "불안정한");
    }

    #[test]
    fn parse_response_preserves_standalone_whitespace_tokens_across_partitions() {
        let responses = SonioxAdapter.parse_response(
            r#"{
                "tokens": [
                    { "text": "hello", "start_ms": 0, "end_ms": 100, "is_final": true },
                    { "text": " ", "start_ms": 100, "end_ms": 100, "is_final": false },
                    { "text": "world", "start_ms": 100, "end_ms": 200, "is_final": false }
                ]
            }"#,
        );

        assert_eq!(responses.len(), 2);

        let final_alt = match &responses[0] {
            StreamResponse::TranscriptResponse {
                channel, is_final, ..
            } => {
                assert!(*is_final);
                &channel.alternatives[0]
            }
            _ => panic!("expected transcript response"),
        };
        assert_eq!(final_alt.transcript, "hello");
        assert_eq!(final_alt.words.len(), 1);
        assert_eq!(final_alt.words[0].word, "hello");

        let partial_alt = match &responses[1] {
            StreamResponse::TranscriptResponse {
                channel, is_final, ..
            } => {
                assert!(!*is_final);
                &channel.alternatives[0]
            }
            _ => panic!("expected transcript response"),
        };
        assert_eq!(partial_alt.transcript, " world");
        assert_eq!(partial_alt.words.len(), 1);
        assert_eq!(partial_alt.words[0].word, "world");
    }
}

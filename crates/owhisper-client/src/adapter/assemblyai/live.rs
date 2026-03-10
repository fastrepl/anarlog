use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::Deserialize;

use super::AssemblyAIAdapter;
use super::language::STREAMING_LANGUAGES;
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::{WordBuilder, calculate_time_span, ms_to_secs};

// https://www.assemblyai.com/docs/api-reference/streaming-api/streaming-api.md
impl RealtimeSttAdapter for AssemblyAIAdapter {
    fn provider_name(&self) -> &'static str {
        "assemblyai"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        let primary_lang = languages.first().map(|l| l.iso639().code()).unwrap_or("en");
        STREAMING_LANGUAGES.contains(&primary_lang)
    }

    fn supports_native_multichannel(&self) -> bool {
        // https://www.assemblyai.com/docs/universal-streaming/multichannel-streams.md
        false
    }

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, _channels: u8) -> url::Url {
        let (mut url, existing_params) = Self::streaming_ws_url(api_base);

        {
            let mut query_pairs = url.query_pairs_mut();

            for (key, value) in &existing_params {
                query_pairs.append_pair(key, value);
            }

            let sample_rate = params.sample_rate.to_string();
            query_pairs.append_pair("sample_rate", &sample_rate);
            query_pairs.append_pair("encoding", "pcm_s16le");
            query_pairs.append_pair("format_turns", "true");

            let default = crate::providers::Provider::AssemblyAI.default_live_model();
            let model = match params.model.as_deref() {
                Some(m) if crate::providers::is_meta_model(m) => default,
                Some(m) => m,
                None => default,
            };

            let (speech_model, language, language_detection) =
                Self::resolve_language_config(model, params);

            query_pairs.append_pair("speech_model", speech_model);
            query_pairs.append_pair("language", language);
            if language_detection {
                query_pairs.append_pair("language_detection", "true");
            }

            if let Some(custom) = &params.custom_query
                && let Some(max_silence) = custom.get("max_turn_silence")
            {
                query_pairs.append_pair("max_turn_silence", max_silence);
            }

            if !params.keywords.is_empty() {
                let keyterms_json = serde_json::to_string(&params.keywords).unwrap_or_default();
                query_pairs.append_pair("keyterms_prompt", &keyterms_json);
            }
        }

        url
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|k| crate::providers::Provider::AssemblyAI.build_auth_header(k))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn finalize_message(&self) -> Message {
        Message::Text(r#"{"type":"Terminate"}"#.into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let msg: AssemblyAIMessage = match serde_json::from_str(raw) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    error = ?e,
                    hyprnote.payload.size_bytes = raw.len() as u64,
                    "assemblyai_json_parse_failed"
                );
                return vec![];
            }
        };

        match msg {
            AssemblyAIMessage::Begin { id, expires_at } => {
                tracing::debug!(
                    hyprnote.stt.provider_session.id = %id,
                    hyprnote.stt.provider_session.expires_at = %expires_at,
                    "assemblyai_session_began"
                );
                vec![]
            }
            AssemblyAIMessage::Turn(turn) => Self::parse_turn(turn),
            AssemblyAIMessage::Termination {
                audio_duration_seconds,
                session_duration_seconds,
            } => {
                tracing::debug!(
                    hyprnote.audio.duration_s = audio_duration_seconds,
                    hyprnote.stt.provider_session.duration_s = session_duration_seconds,
                    "assemblyai_session_terminated"
                );
                vec![StreamResponse::TerminalResponse {
                    request_id: String::new(),
                    created: String::new(),
                    duration: audio_duration_seconds as f64,
                    channels: 1,
                }]
            }
            AssemblyAIMessage::Error { error } => {
                tracing::error!(error = %error, "assemblyai_error");
                vec![StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: error,
                    provider: "assemblyai".to_string(),
                }]
            }
            AssemblyAIMessage::Unknown => {
                tracing::debug!(
                    hyprnote.payload.size_bytes = raw.len() as u64,
                    "assemblyai_unknown_message"
                );
                vec![]
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AssemblyAIMessage {
    Begin {
        id: String,
        expires_at: u64,
    },
    Turn(TurnMessage),
    Termination {
        audio_duration_seconds: u64,
        session_duration_seconds: u64,
    },
    Error {
        error: String,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct TurnMessage {
    #[serde(default)]
    #[allow(dead_code)]
    turn_order: u32,
    #[serde(default)]
    turn_is_formatted: bool,
    #[serde(default)]
    end_of_turn: bool,
    #[serde(default)]
    transcript: String,
    #[serde(default)]
    utterance: Option<String>,
    #[serde(default)]
    language_code: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    language_confidence: Option<f64>,
    #[serde(default)]
    end_of_turn_confidence: f64,
    #[serde(default)]
    words: Vec<AssemblyAIWord>,
}

#[derive(Debug, Deserialize)]
struct AssemblyAIWord {
    text: String,
    #[serde(default)]
    start: u64,
    #[serde(default)]
    end: u64,
    #[serde(default)]
    confidence: f64,
    #[serde(default)]
    #[allow(dead_code)]
    word_is_final: bool,
}

impl AssemblyAIAdapter {
    fn resolve_language_config(
        model: &str,
        params: &ListenParams,
    ) -> (&'static str, &'static str, bool) {
        let is_multilingual_model =
            matches!(model, "multilingual" | "universal-streaming-multilingual");

        let needs_multilingual = is_multilingual_model
            || params.languages.len() > 1
            || params
                .languages
                .first()
                .map(|l| l.iso639().code() != "en")
                .unwrap_or(false);

        if needs_multilingual {
            ("universal-streaming-multilingual", "multi", true)
        } else {
            ("universal-streaming-english", "en", false)
        }
    }

    fn parse_turn(turn: TurnMessage) -> Vec<StreamResponse> {
        tracing::debug!(
            transcript = %turn.transcript,
            utterance = ?turn.utterance,
            words_len = turn.words.len(),
            turn_is_formatted = turn.turn_is_formatted,
            end_of_turn = turn.end_of_turn,
            "assemblyai_turn_received"
        );

        if turn.transcript.is_empty() && turn.words.is_empty() {
            return vec![];
        }

        let is_final = turn.turn_is_formatted || turn.end_of_turn;
        let speech_final = turn.end_of_turn;
        let from_finalize = false;

        let words: Vec<_> = turn
            .words
            .iter()
            .map(|w| {
                WordBuilder::new(&w.text)
                    .start(ms_to_secs(w.start))
                    .end(ms_to_secs(w.end))
                    .confidence(w.confidence)
                    .language(turn.language_code.clone())
                    .build()
            })
            .collect();

        let (start, duration) = calculate_time_span(&words);

        let transcript = if turn.turn_is_formatted {
            turn.transcript.clone()
        } else if let Some(ref utt) = turn.utterance {
            if !utt.is_empty() {
                utt.clone()
            } else if !turn.transcript.is_empty() {
                turn.transcript.clone()
            } else {
                words
                    .iter()
                    .map(|w| w.word.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        } else if !turn.transcript.is_empty() {
            turn.transcript.clone()
        } else {
            words
                .iter()
                .map(|w| w.word.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        };

        let channel = Channel {
            alternatives: vec![Alternatives {
                transcript,
                words,
                confidence: turn.end_of_turn_confidence,
                languages: turn.language_code.map(|l| vec![l]).unwrap_or_default(),
            }],
        };

        vec![StreamResponse::TranscriptResponse {
            is_final,
            speech_final,
            from_finalize,
            start,
            duration,
            channel,
            metadata: Metadata::default(),
            channel_index: vec![0, 1],
        }]
    }
}

#[cfg(test)]
mod tests {
    use hypr_language::ISO639;

    use super::AssemblyAIAdapter;
    use crate::ListenClient;
    use crate::test_utils::{UrlTestCase, run_dual_test, run_single_test, run_url_test_cases};

    const API_BASE: &str = "https://api.assemblyai.com";

    #[test]
    fn test_english_urls() {
        run_url_test_cases(
            &AssemblyAIAdapter::default(),
            API_BASE,
            &[
                UrlTestCase {
                    name: "english_only",
                    model: None,
                    languages: &[ISO639::En],
                    contains: &["speech_model=universal-streaming-english", "language=en"],
                    not_contains: &["language_detection"],
                },
                UrlTestCase {
                    name: "empty_defaults_to_english",
                    model: None,
                    languages: &[],
                    contains: &["speech_model=universal-streaming-english", "language=en"],
                    not_contains: &["language_detection"],
                },
            ],
        );
    }

    #[test]
    fn test_multilingual_urls() {
        run_url_test_cases(
            &AssemblyAIAdapter::default(),
            API_BASE,
            &[
                UrlTestCase {
                    name: "non_english_single",
                    model: None,
                    languages: &[ISO639::Es],
                    contains: &[
                        "speech_model=universal-streaming-multilingual",
                        "language=multi",
                        "language_detection=true",
                    ],
                    not_contains: &[],
                },
                UrlTestCase {
                    name: "multi_language",
                    model: None,
                    languages: &[ISO639::En, ISO639::Es],
                    contains: &[
                        "speech_model=universal-streaming-multilingual",
                        "language=multi",
                        "language_detection=true",
                    ],
                    not_contains: &[],
                },
                UrlTestCase {
                    name: "explicit_multilingual_model",
                    model: Some("universal-streaming-multilingual"),
                    languages: &[ISO639::En],
                    contains: &[
                        "speech_model=universal-streaming-multilingual",
                        "language=multi",
                        "language_detection=true",
                    ],
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
                    .adapter::<AssemblyAIAdapter>()
                    .api_base("wss://streaming.assemblyai.com")
                    .api_key(
                        std::env::var("ASSEMBLYAI_API_KEY").expect("ASSEMBLYAI_API_KEY not set"),
                    )
                    .params($params)
                    .build_single()
                    .await;
                run_single_test(client, "assemblyai").await;
            }
        };
    }

    single_test!(
        test_build_single,
        owhisper_interface::ListenParams {
            model: Some("universal-streaming-english".to_string()),
            languages: vec![hypr_language::ISO639::En.into()],
            ..Default::default()
        }
    );

    single_test!(
        test_single_with_keywords,
        owhisper_interface::ListenParams {
            model: Some("universal-streaming-english".to_string()),
            languages: vec![hypr_language::ISO639::En.into()],
            keywords: vec!["Hyprnote".to_string(), "transcription".to_string()],
            ..Default::default()
        }
    );

    single_test!(
        test_single_multi_lang_1,
        owhisper_interface::ListenParams {
            model: Some("universal-streaming-multilingual".to_string()),
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
            model: Some("universal-streaming-multilingual".to_string()),
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
            .adapter::<AssemblyAIAdapter>()
            .api_base("wss://streaming.assemblyai.com")
            .api_key(std::env::var("ASSEMBLYAI_API_KEY").expect("ASSEMBLYAI_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                model: Some("universal-streaming-english".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_dual()
            .await;

        run_dual_test(client, "assemblyai").await;
    }
}

use anlg_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::Deserialize;

use super::AssemblyAIAdapter;
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::{WordBuilder, calculate_time_span, ms_to_secs};

// https://www.assemblyai.com/docs/api-reference/streaming-api/streaming-api.md
impl RealtimeSttAdapter for AssemblyAIAdapter {
    fn provider_name(&self) -> &'static str {
        "assemblyai"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> bool {
        languages.is_empty() || Self::language_support_live(languages).is_supported()
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
            query_pairs.append_pair("speech_model", LIVE_SPEECH_MODEL);
            if params.languages.len() > 1 {
                query_pairs.append_pair("language_detection", "true");
            }

            if let Some(custom) = &params.custom_query
                && let Some(max_silence) = custom.get("max_turn_silence")
            {
                query_pairs.append_pair("max_turn_silence", max_silence);
            }

            // Diarization is always on, as with every other live provider and the
            // AssemblyAI batch path; participant hints only bound the speaker count.
            query_pairs.append_pair("speaker_labels", "true");
            if let Some(max_speakers) = Self::streaming_max_speakers(params) {
                query_pairs.append_pair("max_speakers", &max_speakers.to_string());
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
                    anarlog.payload.size_bytes = raw.len() as u64,
                    "assemblyai_json_parse_failed"
                );
                return vec![];
            }
        };

        match msg {
            AssemblyAIMessage::Begin { id, expires_at } => {
                tracing::debug!(
                    anarlog.stt.provider_session.id = %id,
                    anarlog.stt.provider_session.expires_at = %expires_at,
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
                    anarlog.audio.duration_s = audio_duration_seconds,
                    anarlog.stt.provider_session.duration_s = session_duration_seconds,
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
                    anarlog.payload.size_bytes = raw.len() as u64,
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
    speaker_label: Option<String>,
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

// The streaming API only accepts universal-3-5-pro and the cheaper
// universal-streaming-* tiers; retired u3-rt-pro / universal-3-pro selections
// and the removed whisper-rt fallback all land here.
const LIVE_SPEECH_MODEL: &str = "universal-3-5-pro";

impl AssemblyAIAdapter {
    fn streaming_max_speakers(params: &ListenParams) -> Option<u32> {
        params.max_speakers.or(params.num_speakers).or_else(|| {
            params
                .custom_query
                .as_ref()
                .and_then(|custom| custom.get("max_speakers"))
                .and_then(|value| value.parse().ok())
        })
    }

    fn parse_speaker_label(label: Option<&str>) -> Option<i32> {
        let label = label?.trim();
        if label.is_empty() || label.eq_ignore_ascii_case("unknown") {
            return None;
        }

        let upper = label.as_bytes().first().copied()?.to_ascii_uppercase();
        if !upper.is_ascii_uppercase() {
            return None;
        }

        Some((upper - b'A') as i32)
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
        let speaker = Self::parse_speaker_label(turn.speaker_label.as_deref());

        let words: Vec<_> = turn
            .words
            .iter()
            .filter(|w| w.word_is_final)
            .map(|w| {
                WordBuilder::new(&w.text)
                    .start(ms_to_secs(w.start))
                    .end(ms_to_secs(w.end))
                    .confidence(w.confidence)
                    .speaker(speaker)
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
    use anlg_language::ISO639;
    use owhisper_interface::stream::StreamResponse;

    use super::{AssemblyAIAdapter, AssemblyAIWord, TurnMessage};
    use crate::ListenClient;
    use crate::adapter::RealtimeSttAdapter;
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
                    contains: &["speech_model=universal-3-5-pro"],
                    not_contains: &["format_turns", "language=", "language_detection"],
                },
                UrlTestCase {
                    name: "empty_defaults_to_english",
                    model: None,
                    languages: &[],
                    contains: &["speech_model=universal-3-5-pro"],
                    not_contains: &["format_turns", "language=", "language_detection"],
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
                    name: "explicit_supported_language_keeps_u35",
                    model: Some("universal-3-5-pro-realtime"),
                    languages: &[ISO639::Es],
                    contains: &["speech_model=universal-3-5-pro"],
                    not_contains: &["format_turns", "language=", "speech_model=whisper-rt"],
                },
                UrlTestCase {
                    name: "retired_u3_model_routes_to_u35",
                    model: Some("u3-rt-pro"),
                    languages: &[ISO639::Es],
                    contains: &["speech_model=universal-3-5-pro"],
                    not_contains: &[
                        "format_turns",
                        "language=",
                        "speech_model=whisper-rt",
                        "speech_model=u3-rt-pro",
                    ],
                },
                UrlTestCase {
                    name: "supported_multi_language_keeps_u35",
                    model: None,
                    languages: &[ISO639::En, ISO639::Es],
                    contains: &["speech_model=universal-3-5-pro", "language_detection=true"],
                    not_contains: &["format_turns", "language=", "speech_model=whisper-rt"],
                },
                UrlTestCase {
                    name: "u35_language_outside_legacy_u3_keeps_u35",
                    model: Some("universal-3-5-pro-realtime"),
                    languages: &[ISO639::Ja],
                    contains: &["speech_model=universal-3-5-pro"],
                    not_contains: &["format_turns", "language=", "speech_model=whisper-rt"],
                },
                UrlTestCase {
                    name: "removed_whisper_rt_selection_routes_to_u35",
                    model: Some("whisper-rt"),
                    languages: &[ISO639::En],
                    contains: &["speech_model=universal-3-5-pro"],
                    not_contains: &["format_turns", "speech_model=whisper-rt"],
                },
            ],
        );
    }

    #[test]
    fn test_streaming_diarization_query_params() {
        let url = AssemblyAIAdapter.build_ws_url(
            API_BASE,
            &owhisper_interface::ListenParams {
                model: Some("universal-3-5-pro-realtime".to_string()),
                num_speakers: Some(3),
                ..Default::default()
            },
            1,
        );

        let query = url.query().expect("query string");
        assert!(query.contains("speaker_labels=true"));
        assert!(query.contains("max_speakers=3"));
    }

    #[test]
    fn test_streaming_min_speakers_enables_diarization() {
        let url = AssemblyAIAdapter.build_ws_url(
            API_BASE,
            &owhisper_interface::ListenParams {
                model: Some("universal-3-5-pro-realtime".to_string()),
                min_speakers: Some(2),
                ..Default::default()
            },
            1,
        );

        let query = url.query().expect("query string");
        assert!(query.contains("speaker_labels=true"));
        assert!(!query.contains("max_speakers"));
    }

    #[test]
    fn test_streaming_diarization_stays_on_without_speaker_hints() {
        // An ad-hoc meeting has no participant list; remote speakers must
        // still be told apart, just without a cap.
        let url = AssemblyAIAdapter.build_ws_url(
            API_BASE,
            &owhisper_interface::ListenParams {
                model: Some("universal-3-5-pro-realtime".to_string()),
                ..Default::default()
            },
            1,
        );

        let query = url.query().expect("query string");
        assert!(query.contains("speaker_labels=true"));
        assert!(!query.contains("max_speakers"));
    }

    #[test]
    fn test_language_support_live_is_limited_to_universal_3_5_pro_languages() {
        assert!(AssemblyAIAdapter::language_support_live(&[]).is_supported());
        assert!(AssemblyAIAdapter::language_support_live(&[ISO639::Ja.into()]).is_supported());
        assert!(
            AssemblyAIAdapter::language_support_live(&[ISO639::En.into(), ISO639::Es.into()])
                .is_supported()
        );

        // Korean is batch-only (Universal-2); without whisper-rt there is no
        // streaming model for it, so live must report unsupported.
        assert!(!AssemblyAIAdapter::language_support_live(&[ISO639::Ko.into()]).is_supported());
        assert!(
            !AssemblyAIAdapter::language_support_live(&[ISO639::En.into(), ISO639::Ko.into()])
                .is_supported()
        );
        assert!(AssemblyAIAdapter::language_support_batch(&[ISO639::Ko.into()]).is_supported());
    }

    #[test]
    fn parse_turn_maps_speaker_labels_to_word_speakers() {
        let responses = AssemblyAIAdapter::parse_turn(TurnMessage {
            turn_order: 1,
            turn_is_formatted: true,
            end_of_turn: true,
            transcript: "Hello there".to_string(),
            speaker_label: Some("B".to_string()),
            utterance: None,
            language_code: Some("en".to_string()),
            language_confidence: None,
            end_of_turn_confidence: 0.99,
            words: vec![AssemblyAIWord {
                text: "Hello".to_string(),
                start: 0,
                end: 500,
                confidence: 0.9,
                word_is_final: true,
            }],
        });

        let StreamResponse::TranscriptResponse { channel, .. } = &responses[0] else {
            panic!("expected transcript response");
        };

        assert_eq!(channel.alternatives[0].words[0].speaker, Some(1));
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
                    .await
                    .unwrap();
                run_single_test(client, "assemblyai").await;
            }
        };
    }

    single_test!(
        test_build_single,
        owhisper_interface::ListenParams {
            model: Some("universal-3-5-pro".to_string()),
            languages: vec![anlg_language::ISO639::En.into()],
            ..Default::default()
        }
    );

    single_test!(
        test_single_with_keywords,
        owhisper_interface::ListenParams {
            model: Some("universal-3-5-pro".to_string()),
            languages: vec![anlg_language::ISO639::En.into()],
            keywords: vec!["Anarlog".to_string(), "transcription".to_string()],
            ..Default::default()
        }
    );

    single_test!(
        test_single_multi_lang_1,
        owhisper_interface::ListenParams {
            model: Some("universal-3-5-pro".to_string()),
            languages: vec![
                anlg_language::ISO639::En.into(),
                anlg_language::ISO639::Es.into(),
            ],
            ..Default::default()
        }
    );

    single_test!(
        test_single_multi_lang_2,
        owhisper_interface::ListenParams {
            model: Some("universal-3-5-pro".to_string()),
            languages: vec![
                anlg_language::ISO639::En.into(),
                anlg_language::ISO639::Ja.into(),
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
                model: Some("universal-3-5-pro".to_string()),
                languages: vec![anlg_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_dual()
            .await
            .unwrap();

        run_dual_test(client, "assemblyai").await;
    }
}

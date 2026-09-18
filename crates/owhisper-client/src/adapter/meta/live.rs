use std::collections::HashMap;

use anlg_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::Deserialize;

use super::{Authorization, MODEL, MetaAdapter, SessionConfig, language};
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::{WordBuilder, calculate_time_span, ms_to_secs};

#[derive(Default)]
pub(super) struct LiveState {
    turns: HashMap<String, Turn>,
    open_turn: Option<String>,
    speaker: Option<i32>,
    labels: Vec<String>,
}

struct Turn {
    start_ms: u64,
    end_ms: Option<u64>,
    speaker: Option<i32>,
}

// https://dev.meta.ai/docs/api-reference/voice/realtime
#[derive(Deserialize)]
#[serde(tag = "type")]
enum Event {
    #[serde(rename = "transcript")]
    Transcript {
        transcript: String,
        #[serde(rename = "audioProcessedMs")]
        audio_processed_ms: u64,
    },
    #[serde(rename = "speechStart")]
    SpeechStart {
        #[serde(rename = "turnId")]
        turn_id: serde_json::Value,
        #[serde(rename = "audioProcessedMs")]
        audio_processed_ms: u64,
    },
    #[serde(rename = "speechEnd")]
    SpeechEnd {
        #[serde(rename = "turnId")]
        turn_id: serde_json::Value,
        #[serde(rename = "audioProcessedMs")]
        audio_processed_ms: u64,
    },
    #[serde(rename = "speechComplete")]
    SpeechComplete {
        #[serde(rename = "turnId")]
        turn_id: serde_json::Value,
        transcript: String,
        #[serde(rename = "audioProcessedMs")]
        audio_processed_ms: u64,
    },
    #[serde(rename = "speaker")]
    Speaker { label: String },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(other)]
    Unknown,
}

impl RealtimeSttAdapter for MetaAdapter {
    fn fork_session(&self) -> Self {
        Self::default()
    }

    fn provider_name(&self) -> &'static str {
        "meta"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> bool {
        language::all_supported(languages)
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

    // The realtime endpoint ignores HTTP headers; the key rides in the handshake.
    fn build_auth_header(&self, _api_key: Option<&str>) -> Option<(&'static str, String)> {
        None
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn initial_message(
        &self,
        api_key: Option<&str>,
        params: &ListenParams,
        _channels: u8,
    ) -> Option<Message> {
        let cfg = SessionConfig {
            authorization: Some(Authorization {
                access_token: format!("Bearer {}", api_key.unwrap_or("")),
            }),
            audio_encoding: match params.sample_rate {
                24000 => "PCM_24KHZ",
                _ => "PCM_16KHZ",
            },
            model: MODEL,
            mode: "DIARIZATION",
            partial_mode: Some("CUMULATIVE"),
            emit_audio_progress: Some(false),
            language_bias: language::language_bias(&params.languages),
            keywords: &params.keywords,
        };
        Some(Message::Text(serde_json::to_string(&cfg).unwrap().into()))
    }

    fn finalize_message(&self) -> Message {
        Message::Text(r#"{"type":"endStream"}"#.into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let event: Event = match serde_json::from_str(raw) {
            Ok(event) => event,
            Err(_) => {
                // The session ack `{"sessionId":...}` is the only untyped frame.
                tracing::debug!(
                    anarlog.payload.size_bytes = raw.len() as u64,
                    "meta_untyped_frame"
                );
                return vec![];
            }
        };

        let mut state = self.live.lock().unwrap_or_else(|e| e.into_inner());

        match event {
            Event::SpeechStart {
                turn_id,
                audio_processed_ms,
            } => {
                let id = turn_key(&turn_id);
                state.turns.insert(
                    id.clone(),
                    Turn {
                        start_ms: audio_processed_ms,
                        end_ms: None,
                        speaker: None,
                    },
                );
                state.open_turn = Some(id);
                vec![]
            }
            Event::Transcript {
                transcript,
                audio_processed_ms,
            } => {
                let start_ms = state
                    .open_turn
                    .as_ref()
                    .and_then(|id| state.turns.get(id))
                    .map(|turn| turn.start_ms)
                    .unwrap_or(audio_processed_ms);
                // ponytail: the speaker label arrives after the span, so partials
                // carry the previous turn's label; finals use the turn's own label.
                build_response(
                    &transcript,
                    start_ms,
                    audio_processed_ms,
                    state.speaker,
                    false,
                )
            }
            Event::SpeechEnd {
                turn_id,
                audio_processed_ms,
            } => {
                let id = turn_key(&turn_id);
                if let Some(turn) = state.turns.get_mut(&id) {
                    turn.end_ms = Some(audio_processed_ms);
                }
                vec![]
            }
            Event::Speaker { label } => {
                let index = match state.labels.iter().position(|known| *known == label) {
                    Some(index) => index,
                    None => {
                        state.labels.push(label);
                        state.labels.len() - 1
                    }
                } as i32;
                state.speaker = Some(index);
                // Observed order: speaker, speechEnd, speechComplete, all for the
                // turn that is still open, so the label belongs to that turn.
                if let Some(turn) = state
                    .open_turn
                    .clone()
                    .and_then(|id| state.turns.get_mut(&id))
                {
                    turn.speaker = Some(index);
                }
                vec![]
            }
            Event::SpeechComplete {
                turn_id,
                transcript,
                audio_processed_ms,
            } => {
                let id = turn_key(&turn_id);
                let turn = state.turns.remove(&id);
                let start_ms = turn
                    .as_ref()
                    .map(|t| t.start_ms)
                    .unwrap_or(audio_processed_ms);
                let end_ms = turn
                    .as_ref()
                    .and_then(|t| t.end_ms)
                    .unwrap_or(audio_processed_ms);
                let speaker = turn.and_then(|t| t.speaker).or(state.speaker);
                build_response(&transcript, start_ms, end_ms, speaker, true)
            }
            Event::Error { message } => {
                crate::log_provider_failure("meta", "provider_error", None, &message);
                vec![StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: message,
                    provider: "meta".to_string(),
                }]
            }
            Event::Unknown => vec![],
        }
    }
}

fn turn_key(turn_id: &serde_json::Value) -> String {
    match turn_id {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn build_response(
    text: &str,
    start_ms: u64,
    end_ms: u64,
    speaker: Option<i32>,
    is_final: bool,
) -> Vec<StreamResponse> {
    let words: Vec<_> = MetaAdapter::word_spans(text, ms_to_secs(start_ms), ms_to_secs(end_ms))
        .into_iter()
        .map(|(word, start, end)| {
            WordBuilder::new(word)
                .start(start)
                .end(end)
                .confidence(1.0)
                .speaker(speaker)
                .build()
        })
        .collect();
    if words.is_empty() {
        return vec![];
    }
    let (start, duration) = calculate_time_span(&words);

    vec![StreamResponse::TranscriptResponse {
        is_final,
        speech_final: is_final,
        from_finalize: false,
        start,
        duration,
        channel: Channel {
            alternatives: vec![Alternatives {
                transcript: text.trim().to_string(),
                words,
                confidence: 1.0,
                languages: vec![],
            }],
        },
        metadata: Metadata::default(),
        channel_index: vec![0, 1],
    }]
}

#[cfg(test)]
mod tests {
    use anlg_ws_client::client::Message;
    use owhisper_interface::stream::StreamResponse;

    use super::MetaAdapter;
    use crate::ListenClient;
    use crate::adapter::RealtimeSttAdapter;
    use crate::test_utils::{run_dual_test, run_single_test};

    fn handshake_json(params: &owhisper_interface::ListenParams) -> serde_json::Value {
        match MetaAdapter::default()
            .initial_message(Some("test_key"), params, 1)
            .unwrap()
        {
            Message::Text(text) => serde_json::from_str(&text).unwrap(),
            _ => panic!("expected text message"),
        }
    }

    fn transcript(response: &StreamResponse) -> (bool, f64, f64, Vec<(String, Option<i32>)>) {
        match response {
            StreamResponse::TranscriptResponse {
                is_final,
                start,
                duration,
                channel,
                ..
            } => (
                *is_final,
                *start,
                *duration,
                channel.alternatives[0]
                    .words
                    .iter()
                    .map(|w| (w.word.clone(), w.speaker))
                    .collect(),
            ),
            _ => panic!("expected transcript response"),
        }
    }

    #[test]
    fn handshake_carries_key_model_mode_and_bias() {
        let json = handshake_json(&owhisper_interface::ListenParams {
            languages: vec![anlg_language::ISO639::En.into()],
            keywords: vec!["BlackMushi".to_string()],
            ..Default::default()
        });

        assert_eq!(json["authorization"]["accessToken"], "Bearer test_key");
        assert_eq!(json["audioEncoding"], "PCM_16KHZ");
        assert_eq!(json["model"], "muse-voice-transcribe-1.0");
        assert_eq!(json["mode"], "DIARIZATION");
        assert_eq!(json["partialMode"], "CUMULATIVE");
        assert_eq!(json["emitAudioProgress"], false);
        assert_eq!(json["languageBias"], serde_json::json!(["English"]));
        assert_eq!(json["keywords"], serde_json::json!(["BlackMushi"]));
    }

    #[test]
    fn handshake_omits_empty_bias_and_keywords() {
        let json = handshake_json(&owhisper_interface::ListenParams::default());
        assert!(json.get("languageBias").is_none());
        assert!(json.get("keywords").is_none());
    }

    #[test]
    fn events_become_partials_then_finals_with_speakers() {
        let adapter = MetaAdapter::default();

        assert!(adapter.parse_response(r#"{"sessionId":"abc"}"#).is_empty());
        assert!(
            adapter
                .parse_response(r#"{"type":"speechStart","turnId":1,"audioProcessedMs":1000}"#)
                .is_empty()
        );

        let partial = adapter.parse_response(
            r#"{"type":"transcript","transcript":"hello there","final":false,"audioProcessedMs":2000}"#,
        );
        assert_eq!(partial.len(), 1);
        let (is_final, start, duration, words) = transcript(&partial[0]);
        assert!(!is_final);
        assert_eq!((start, duration), (1.0, 1.0));
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].1, None);

        assert!(
            adapter
                .parse_response(r#"{"type":"speaker","label":"A","audioProcessedMs":3500}"#)
                .is_empty()
        );
        assert!(
            adapter
                .parse_response(r#"{"type":"speechEnd","turnId":1,"audioProcessedMs":3000}"#)
                .is_empty()
        );
        assert!(
            adapter
                .parse_response(r#"{"type":"speechStart","turnId":2,"audioProcessedMs":3500}"#)
                .is_empty()
        );

        let final_turn = adapter.parse_response(
            r#"{"type":"speechComplete","turnId":1,"transcript":"Hello there.","audioProcessedMs":3600}"#,
        );
        let (is_final, start, duration, words) = transcript(&final_turn[0]);
        assert!(is_final);
        assert_eq!((start, duration), (1.0, 2.0));
        assert_eq!(
            words,
            vec![
                ("Hello".to_string(), Some(0)),
                ("there.".to_string(), Some(0))
            ]
        );

        adapter.parse_response(r#"{"type":"speaker","label":"B","audioProcessedMs":5500}"#);
        adapter.parse_response(r#"{"type":"speechEnd","turnId":2,"audioProcessedMs":5000}"#);
        let second = adapter.parse_response(
            r#"{"type":"speechComplete","turnId":2,"transcript":"Hi.","audioProcessedMs":5100}"#,
        );
        let (_, start, duration, words) = transcript(&second[0]);
        assert_eq!((start, duration), (3.5, 1.5));
        assert_eq!(words[0].1, Some(1));

        assert!(
            adapter
                .parse_response(r#"{"type":"audioProgress","audioProcessedMs":6000}"#)
                .is_empty()
        );
        assert!(
            adapter
                .fork_session()
                .parse_response(
                    r#"{"type":"speechComplete","turnId":9,"transcript":"x","audioProcessedMs":10}"#,
                )
                .iter()
                .all(|r| transcript(r).3[0].1.is_none())
        );
    }

    #[test]
    fn error_event_maps_to_error_response() {
        let responses = MetaAdapter::default()
            .parse_response(r#"{"type":"error","message":"bad key","sessionId":"abc"}"#);
        assert!(matches!(
            &responses[0],
            StreamResponse::ErrorResponse { provider, error_message, .. }
                if provider == "meta" && error_message == "bad key"
        ));
        assert!(matches!(
            MetaAdapter::default().finalize_message(),
            Message::Text(text) if text == r#"{"type":"endStream"}"#
        ));
    }

    #[tokio::test]
    #[ignore]
    async fn test_build_single() {
        let client = ListenClient::builder()
            .adapter::<MetaAdapter>()
            .api_base("https://api.meta.ai/v1")
            .api_key(std::env::var("META_API_KEY").expect("META_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                languages: vec![anlg_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_single()
            .await
            .unwrap();
        run_single_test(client, "meta").await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_build_dual() {
        let client = ListenClient::builder()
            .adapter::<MetaAdapter>()
            .api_base("https://api.meta.ai/v1")
            .api_key(std::env::var("META_API_KEY").expect("META_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                languages: vec![anlg_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_dual()
            .await
            .unwrap();
        run_dual_test(client, "meta").await;
    }
}

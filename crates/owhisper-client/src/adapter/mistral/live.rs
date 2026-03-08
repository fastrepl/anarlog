use std::sync::atomic::Ordering;

use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::{Deserialize, Serialize};

use super::MistralAdapter;
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::WordBuilder;

impl RealtimeSttAdapter for MistralAdapter {
    fn provider_name(&self) -> &'static str {
        "mistral"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        MistralAdapter::is_supported_languages_live(languages)
    }

    fn supports_native_multichannel(&self) -> bool {
        false
    }

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, _channels: u8) -> url::Url {
        let (mut url, existing_params) = Self::build_ws_url_from_base(api_base);

        let default = crate::providers::Provider::Mistral.default_live_model();
        let model = match params.model.as_deref() {
            Some(m) if crate::providers::is_meta_model(m) => default,
            Some("voxtral-mini-2602" | "voxtral-mini-latest") => default,
            Some(m) => m,
            None => default,
        };

        {
            let mut query_pairs = url.query_pairs_mut();
            query_pairs.append_pair("model", model);
            for (key, value) in &existing_params {
                query_pairs.append_pair(key, value);
            }
        }

        url
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|k| crate::providers::Provider::Mistral.build_auth_header(k))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn audio_to_message(&self, audio: bytes::Bytes) -> Message {
        use base64::Engine;
        let base64_audio = base64::engine::general_purpose::STANDARD.encode(&audio);
        let event = InputAudioAppend {
            event_type: "input_audio.append".to_string(),
            audio: base64_audio,
        };
        Message::Text(serde_json::to_string(&event).unwrap().into())
    }

    fn initial_message(
        &self,
        _api_key: Option<&str>,
        params: &ListenParams,
        _channels: u8,
    ) -> Option<Message> {
        let encoding = "pcm_s16le".to_string();
        let sample_rate = params.sample_rate;

        let session_update = SessionUpdateEvent {
            event_type: "session.update".to_string(),
            session: SessionUpdateConfig {
                audio_format: AudioFormatConfig {
                    encoding,
                    sample_rate,
                },
            },
        };

        let json = serde_json::to_string(&session_update).ok()?;
        tracing::debug!(hyprnote.payload.raw = %json, "mistral_session_update_payload");
        Some(Message::Text(json.into()))
    }

    fn finalize_message(&self) -> Message {
        let end = InputAudioEnd {
            event_type: "input_audio.end".to_string(),
        };
        Message::Text(serde_json::to_string(&end).unwrap().into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let event: MistralEvent = match serde_json::from_str(raw) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(
                    error.message = ?e,
                    hyprnote.payload.raw = raw,
                    "mistral_json_parse_failed"
                );
                return vec![];
            }
        };

        match event {
            MistralEvent::SessionCreated { .. } => {
                self.word_counter.store(0, Ordering::Relaxed);
                tracing::debug!("mistral_session_created");
                vec![]
            }
            MistralEvent::SessionUpdated { .. } => {
                tracing::debug!("mistral_session_updated");
                vec![]
            }
            MistralEvent::TranscriptionLanguage { audio_language } => {
                tracing::debug!(
                    hyprnote.stt.language_code = %audio_language,
                    "mistral_transcription_language"
                );
                vec![]
            }
            MistralEvent::TranscriptionTextDelta { text } => {
                tracing::debug!(hyprnote.transcript.text = %text, "mistral_transcription_text_delta");
                self.build_delta_response(&text)
            }
            MistralEvent::TranscriptionSegment {
                text, start, end, ..
            } => {
                tracing::debug!(
                    hyprnote.transcript.text = %text,
                    hyprnote.segment.start_s = start,
                    hyprnote.segment.end_s = end,
                    "mistral_transcription_segment"
                );
                Self::build_segment_response(&text, start, end)
            }
            MistralEvent::TranscriptionDone { .. } => {
                tracing::debug!("mistral_transcription_done");
                vec![]
            }
            MistralEvent::Error { error } => {
                tracing::error!(
                    error.code = error.code,
                    error.message = %error.message,
                    "mistral_error"
                );
                vec![StreamResponse::ErrorResponse {
                    error_code: Some(error.code),
                    error_message: error.message,
                    provider: "mistral".to_string(),
                }]
            }
            MistralEvent::Unknown => {
                tracing::debug!(hyprnote.payload.raw = raw, "mistral_unknown_event");
                vec![]
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct SessionUpdateEvent {
    #[serde(rename = "type")]
    event_type: String,
    session: SessionUpdateConfig,
}

#[derive(Debug, Serialize)]
struct SessionUpdateConfig {
    audio_format: AudioFormatConfig,
}

#[derive(Debug, Serialize)]
struct AudioFormatConfig {
    encoding: String,
    sample_rate: u32,
}

#[derive(Debug, Serialize)]
struct InputAudioAppend {
    #[serde(rename = "type")]
    event_type: String,
    audio: String,
}

#[derive(Debug, Serialize)]
struct InputAudioEnd {
    #[serde(rename = "type")]
    event_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum MistralEvent {
    #[serde(rename = "session.created")]
    SessionCreated { session: SessionInfo },
    #[serde(rename = "session.updated")]
    SessionUpdated { session: SessionInfo },
    #[serde(rename = "transcription.language")]
    TranscriptionLanguage { audio_language: String },
    #[serde(rename = "transcription.text.delta")]
    TranscriptionTextDelta { text: String },
    #[serde(rename = "transcription.segment")]
    TranscriptionSegment {
        text: String,
        start: f64,
        end: f64,
        speaker_id: Option<String>,
    },
    #[serde(rename = "transcription.done")]
    TranscriptionDone {
        model: Option<String>,
        text: Option<String>,
    },
    #[serde(rename = "error")]
    Error { error: MistralError },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SessionInfo {
    request_id: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MistralError {
    message: String,
    code: i32,
}

impl MistralAdapter {
    fn build_delta_response(&self, text: &str) -> Vec<StreamResponse> {
        let words: Vec<_> = text
            .split_whitespace()
            .map(|word| {
                let idx = self.word_counter.fetch_add(1, Ordering::Relaxed);
                let start = idx as f64;
                let end = (idx + 1) as f64;
                WordBuilder::new(word)
                    .start(start)
                    .end(end)
                    .confidence(1.0)
                    .build()
            })
            .collect();

        if words.is_empty() {
            return vec![];
        }

        let transcript: String = text.split_whitespace().collect::<Vec<_>>().join(" ");

        let start = words.first().map(|w| w.start).unwrap_or(0.0);
        let end = words.last().map(|w| w.end).unwrap_or(0.0);

        let channel = Channel {
            alternatives: vec![Alternatives {
                transcript,
                words,
                confidence: 1.0,
                languages: vec![],
            }],
        };

        vec![StreamResponse::TranscriptResponse {
            is_final: true,
            speech_final: false,
            from_finalize: false,
            start,
            duration: end - start,
            channel,
            metadata: Metadata::default(),
            channel_index: vec![0, 1],
        }]
    }

    fn build_segment_response(text: &str, start: f64, end: f64) -> Vec<StreamResponse> {
        if text.is_empty() {
            return vec![];
        }

        let duration = end - start;
        let word_count = text.split_whitespace().count();
        let words: Vec<_> = if word_count > 0 {
            let word_duration = duration / word_count as f64;
            text.split_whitespace()
                .enumerate()
                .map(|(i, word)| {
                    let word_start = start + (i as f64 * word_duration);
                    let word_end = word_start + word_duration;
                    WordBuilder::new(word)
                        .start(word_start)
                        .end(word_end)
                        .confidence(1.0)
                        .build()
                })
                .collect()
        } else {
            vec![]
        };

        let channel = Channel {
            alternatives: vec![Alternatives {
                transcript: text.to_string(),
                words,
                confidence: 1.0,
                languages: vec![],
            }],
        };

        vec![StreamResponse::TranscriptResponse {
            is_final: true,
            speech_final: true,
            from_finalize: false,
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

    use super::MistralAdapter;
    use crate::ListenClient;
    use crate::adapter::RealtimeSttAdapter;
    use crate::test_utils::{
        UrlTestCase, run_dual_test_with_rate, run_single_test_with_rate, run_url_test_cases,
    };
    use owhisper_interface::stream::StreamResponse;

    const API_BASE: &str = "wss://api.mistral.ai";
    const MISTRAL_SAMPLE_RATE: u32 = 16000;

    #[test]
    fn test_base_url() {
        run_url_test_cases(
            &MistralAdapter::default(),
            API_BASE,
            &[UrlTestCase {
                name: "base_url_structure",
                model: None,
                languages: &[ISO639::En],
                contains: &["api.mistral.ai"],
                not_contains: &[],
            }],
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_build_single() {
        let client = ListenClient::builder()
            .adapter::<MistralAdapter>()
            .api_base("wss://api.mistral.ai")
            .api_key(std::env::var("MISTRAL_API_KEY").expect("MISTRAL_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                languages: vec![hypr_language::ISO639::En.into()],
                sample_rate: MISTRAL_SAMPLE_RATE,
                ..Default::default()
            })
            .build_single()
            .await;

        run_single_test_with_rate(client, "mistral", MISTRAL_SAMPLE_RATE).await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_build_dual() {
        let client = ListenClient::builder()
            .adapter::<MistralAdapter>()
            .api_base("wss://api.mistral.ai")
            .api_key(std::env::var("MISTRAL_API_KEY").expect("MISTRAL_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                languages: vec![hypr_language::ISO639::En.into()],
                sample_rate: MISTRAL_SAMPLE_RATE,
                ..Default::default()
            })
            .build_dual()
            .await;

        run_dual_test_with_rate(client, "mistral", MISTRAL_SAMPLE_RATE).await;
    }

    #[test]
    fn test_parse_session_created() {
        let adapter = MistralAdapter::default();
        let raw = r#"{"type":"session.created","session":{"request_id":"abc123","model":"voxtral-mini-transcribe-realtime-2602","audio_format":{"encoding":"pcm_s16le","sample_rate":16000}}}"#;
        let responses = adapter.parse_response(raw);
        assert!(responses.is_empty());
    }

    #[test]
    fn test_parse_text_delta() {
        let adapter = MistralAdapter::default();

        let responses =
            adapter.parse_response(r#"{"type":"transcription.text.delta","text":" Maybe"}"#);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            StreamResponse::TranscriptResponse {
                is_final, channel, ..
            } => {
                assert!(is_final);
                assert_eq!(channel.alternatives[0].transcript, "Maybe");
                let words = &channel.alternatives[0].words;
                assert_eq!(words.len(), 1);
                assert_eq!(words[0].start, 0.0);
                assert_eq!(words[0].end, 1.0);
            }
            _ => panic!("expected TranscriptResponse"),
        }

        let responses =
            adapter.parse_response(r#"{"type":"transcription.text.delta","text":" this"}"#);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            StreamResponse::TranscriptResponse { channel, .. } => {
                let words = &channel.alternatives[0].words;
                assert_eq!(words.len(), 1);
                assert_eq!(words[0].start, 1.0);
                assert_eq!(words[0].end, 2.0);
            }
            _ => panic!("expected TranscriptResponse"),
        }
    }

    #[test]
    fn test_parse_segment() {
        let adapter = MistralAdapter::default();
        let raw = r#"{"type":"transcription.segment","text":"hello world","start":1.0,"end":2.5}"#;
        let responses = adapter.parse_response(raw);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            StreamResponse::TranscriptResponse {
                is_final,
                start,
                duration,
                channel,
                ..
            } => {
                assert!(is_final);
                assert_eq!(*start, 1.0);
                assert_eq!(*duration, 1.5);
                assert_eq!(channel.alternatives[0].transcript, "hello world");
                assert_eq!(channel.alternatives[0].words.len(), 2);
            }
            _ => panic!("expected TranscriptResponse"),
        }
    }

    #[test]
    fn test_parse_error() {
        let adapter = MistralAdapter::default();
        let raw = r#"{"type":"error","error":{"message":"invalid request","code":400}}"#;
        let responses = adapter.parse_response(raw);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            StreamResponse::ErrorResponse {
                error_message,
                error_code,
                provider,
            } => {
                assert_eq!(error_message, "invalid request");
                assert_eq!(*error_code, Some(400));
                assert_eq!(provider, "mistral");
            }
            _ => panic!("expected ErrorResponse"),
        }
    }
}

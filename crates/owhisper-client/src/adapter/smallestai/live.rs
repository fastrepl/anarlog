use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::Deserialize;

use super::SmallestAIAdapter;
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::{WordBuilder, calculate_time_span, parse_speaker_id};

impl RealtimeSttAdapter for SmallestAIAdapter {
    fn provider_name(&self) -> &'static str {
        "smallestai"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        SmallestAIAdapter::is_supported_languages_live(languages, model)
    }

    fn supports_native_multichannel(&self) -> bool {
        false
    }

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, _channels: u8) -> url::Url {
        let (mut url, existing_params) = SmallestAIAdapter::build_ws_url_from_base(api_base);
        {
            let mut query_pairs = url.query_pairs_mut();

            for (key, value) in &existing_params {
                query_pairs.append_pair(key, value);
            }

            query_pairs.append_pair("encoding", "linear16");
            query_pairs.append_pair("sample_rate", &params.sample_rate.to_string());
            query_pairs.append_pair("word_timestamps", "true");
            query_pairs.append_pair("diarize", "true");
            query_pairs.append_pair("full_transcript", "true");
            query_pairs.append_pair(
                "language",
                &SmallestAIAdapter::language_query_value(&params.languages),
            );
        }

        url
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.map(|key| ("Authorization", format!("Bearer {key}")))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn finalize_message(&self) -> Message {
        Message::Text(r#"{"type":"finalize"}"#.into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let msg: SmallestRealtimeMessage = match serde_json::from_str(raw) {
            Ok(msg) => msg,
            Err(error) => {
                tracing::warn!(
                    error = ?error,
                    hyprnote.payload.size_bytes = raw.len() as u64,
                    "smallestai_json_parse_failed"
                );
                return vec![];
            }
        };

        if let Some(message) = msg.error_message() {
            return vec![StreamResponse::ErrorResponse {
                error_code: None,
                error_message: message,
                provider: "smallestai".to_string(),
            }];
        }

        let transcript = msg
            .transcript
            .clone()
            .or(msg.text.clone())
            .or(msg.full_transcript.clone())
            .unwrap_or_default();

        if transcript.is_empty() && msg.words.is_empty() && msg.utterances.is_empty() {
            return vec![];
        }

        let words: Vec<_> = msg
            .words
            .iter()
            .map(SmallestRealtimeWord::to_word)
            .collect();
        let (start, duration) = if words.is_empty() {
            match (msg.utterances.first(), msg.utterances.last()) {
                (Some(first), Some(last)) => (first.start, last.end - first.start),
                _ => (0.0, 0.0),
            }
        } else {
            calculate_time_span(&words)
        };

        let mut languages = msg.languages.clone().unwrap_or_default();
        if languages.is_empty()
            && let Some(language) = &msg.language
        {
            languages.push(language.clone());
        }

        let channel = Channel {
            alternatives: vec![Alternatives {
                transcript,
                words,
                confidence: 1.0,
                languages,
            }],
        };

        let is_final = msg.is_final || msg.is_last;

        vec![StreamResponse::TranscriptResponse {
            is_final,
            speech_final: is_final,
            from_finalize: msg.from_finalize || msg.is_last,
            start,
            duration,
            channel,
            metadata: Metadata::default(),
            channel_index: vec![0, 1],
        }]
    }
}

#[derive(Debug, Default, Deserialize)]
struct SmallestRealtimeMessage {
    #[serde(default, rename = "type")]
    message_type: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default, rename = "session_id")]
    _session_id: Option<String>,
    #[serde(default)]
    transcript: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    full_transcript: Option<String>,
    #[serde(default)]
    is_final: bool,
    #[serde(default)]
    is_last: bool,
    #[serde(default)]
    from_finalize: bool,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    languages: Option<Vec<String>>,
    #[serde(default)]
    words: Vec<SmallestRealtimeWord>,
    #[serde(default)]
    utterances: Vec<SmallestUtterance>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    error: Option<SmallestMessageError>,
    #[serde(default)]
    detail: Option<String>,
}

impl SmallestRealtimeMessage {
    fn error_message(&self) -> Option<String> {
        if let Some(error) = &self.error {
            return Some(error.to_message());
        }

        if matches!(self.status.as_deref(), Some("error" | "failed")) {
            return self
                .message
                .clone()
                .or(self.detail.clone())
                .or_else(|| self.message_type.clone());
        }

        None
    }
}

#[derive(Debug, Default, Deserialize)]
struct SmallestUtterance {
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
}

#[derive(Debug, Default, Deserialize)]
struct SmallestRealtimeWord {
    #[serde(default)]
    word: String,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    speaker: Option<SmallestSpeaker>,
    #[serde(default)]
    language: Option<String>,
}

impl SmallestRealtimeWord {
    fn to_word(&self) -> owhisper_interface::stream::Word {
        WordBuilder::new(&self.word)
            .start(self.start)
            .end(self.end)
            .confidence(self.confidence.unwrap_or(1.0))
            .speaker(self.speaker.as_ref().and_then(SmallestSpeaker::as_i32))
            .language(self.language.clone())
            .build()
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SmallestSpeaker {
    Int(i32),
    String(String),
}

impl SmallestSpeaker {
    fn as_i32(&self) -> Option<i32> {
        match self {
            Self::Int(value) => Some(*value),
            Self::String(value) => parse_speaker_id(value).and_then(|value| value.try_into().ok()),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SmallestMessageError {
    String(String),
    Object {
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        detail: Option<String>,
        #[serde(default)]
        code: Option<String>,
    },
}

impl SmallestMessageError {
    fn to_message(&self) -> String {
        match self {
            Self::String(message) => message.clone(),
            Self::Object {
                message,
                error,
                detail,
                code,
            } => {
                let message = message
                    .clone()
                    .or(error.clone())
                    .or(detail.clone())
                    .unwrap_or_else(|| "provider error".to_string());

                match code {
                    Some(code) => format!("{code}: {message}"),
                    None => message,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use hypr_language::ISO639;

    use super::*;

    const API_BASE: &str = "https://api.smallest.ai";

    #[test]
    fn test_build_ws_url_single_language() {
        let url = SmallestAIAdapter.build_ws_url(
            API_BASE,
            &ListenParams {
                sample_rate: 16_000,
                languages: vec![ISO639::En.into()],
                ..Default::default()
            },
            1,
        );

        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(
            url.as_str().split('?').next().unwrap(),
            "wss://api.smallest.ai/waves/v1/pulse/get_text"
        );
        assert_eq!(query.get("encoding"), Some(&"linear16".to_string()));
        assert_eq!(query.get("sample_rate"), Some(&"16000".to_string()));
        assert_eq!(query.get("word_timestamps"), Some(&"true".to_string()));
        assert_eq!(query.get("diarize"), Some(&"true".to_string()));
        assert_eq!(query.get("full_transcript"), Some(&"true".to_string()));
        assert_eq!(query.get("language"), Some(&"en".to_string()));
    }

    #[test]
    fn test_build_ws_url_auto_detect() {
        let url = SmallestAIAdapter.build_ws_url(
            API_BASE,
            &ListenParams {
                sample_rate: 16_000,
                languages: vec![ISO639::En.into(), ISO639::Fr.into()],
                ..Default::default()
            },
            1,
        );

        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query.get("language"), Some(&"multi".to_string()));
    }

    #[test]
    fn test_finalize_message() {
        assert_eq!(
            SmallestAIAdapter.finalize_message(),
            Message::Text(r#"{"type":"finalize"}"#.into())
        );
    }

    #[test]
    fn test_parse_partial_response() {
        let responses = SmallestAIAdapter.parse_response(
            r#"{
                "type": "transcription",
                "status": "success",
                "session_id": "sess_123",
                "transcript": "Hello",
                "is_final": false,
                "is_last": false
            }"#,
        );

        assert_eq!(responses.len(), 1);
        match &responses[0] {
            StreamResponse::TranscriptResponse {
                is_final,
                speech_final,
                from_finalize,
                channel,
                ..
            } => {
                assert!(!is_final);
                assert!(!speech_final);
                assert!(!from_finalize);
                assert_eq!(channel.alternatives[0].transcript, "Hello");
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn test_parse_final_response_with_words() {
        let responses = SmallestAIAdapter.parse_response(
            r#"{
                "type": "transcription",
                "status": "success",
                "session_id": "sess_123",
                "transcript": "Hello, world",
                "is_final": true,
                "is_last": false,
                "language": "en",
                "languages": ["en"],
                "words": [
                    {"word": "Hello", "start": 0.0, "end": 0.4, "confidence": 0.99, "speaker": 0, "language": "en"},
                    {"word": "world", "start": 0.5, "end": 0.9, "confidence": 0.98, "speaker": "speaker_1", "language": "en"}
                ]
            }"#,
        );

        match &responses[0] {
            StreamResponse::TranscriptResponse {
                is_final,
                speech_final,
                from_finalize,
                start,
                duration,
                channel,
                ..
            } => {
                assert!(*is_final);
                assert!(*speech_final);
                assert!(!from_finalize);
                assert_eq!(*start, 0.0);
                assert_eq!(*duration, 0.9);
                assert_eq!(channel.alternatives[0].languages, vec!["en".to_string()]);
                assert_eq!(channel.alternatives[0].words.len(), 2);
                assert_eq!(channel.alternatives[0].words[0].speaker, Some(0));
                assert_eq!(channel.alternatives[0].words[1].speaker, Some(1));
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn test_parse_last_response_marks_finalize() {
        let responses = SmallestAIAdapter.parse_response(
            r#"{
                "type": "transcription",
                "status": "success",
                "session_id": "sess_123",
                "transcript": "Goodbye!",
                "is_final": true,
                "is_last": true,
                "full_transcript": "Hello world Goodbye!"
            }"#,
        );

        match &responses[0] {
            StreamResponse::TranscriptResponse {
                is_final,
                speech_final,
                from_finalize,
                ..
            } => {
                assert!(*is_final);
                assert!(*speech_final);
                assert!(*from_finalize);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn test_parse_error_response() {
        let responses = SmallestAIAdapter.parse_response(
            r#"{
                "type": "error",
                "status": "error",
                "message": "invalid request"
            }"#,
        );

        match &responses[0] {
            StreamResponse::ErrorResponse {
                error_message,
                provider,
                ..
            } => {
                assert_eq!(error_message, "invalid request");
                assert_eq!(provider, "smallestai");
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }
}

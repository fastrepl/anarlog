use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::{Deserialize, Serialize};

use super::OpenAIAdapter;
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::{WordBuilder, calculate_time_span};

const VAD_DETECTION_TYPE: &str = "server_vad";
const VAD_THRESHOLD: f32 = 0.5;
const VAD_PREFIX_PADDING_MS: u32 = 300;
const VAD_SILENCE_DURATION_MS: u32 = 500;

impl RealtimeSttAdapter for OpenAIAdapter {
    fn provider_name(&self) -> &'static str {
        "openai"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        OpenAIAdapter::is_supported_languages_live(languages)
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

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|k| crate::providers::Provider::OpenAI.build_auth_header(k))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn audio_to_message(&self, audio: bytes::Bytes) -> Message {
        use base64::Engine;
        let base64_audio = base64::engine::general_purpose::STANDARD.encode(&audio);
        let event = InputAudioBufferAppend {
            event_type: "input_audio_buffer.append".to_string(),
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
        let language = params
            .languages
            .first()
            .map(|l| l.iso639().code().to_string());

        let default = crate::providers::Provider::OpenAI.default_live_model();
        let model = match params.model.as_deref() {
            Some(m) if crate::providers::is_meta_model(m) => default,
            Some(m) => m,
            None => default,
        };

        let session_config = SessionUpdateEvent {
            event_type: "session.update".to_string(),
            session: SessionConfig {
                session_type: "transcription".to_string(),
                audio: Some(AudioConfig {
                    input: Some(AudioInputConfig {
                        format: Some(AudioFormat {
                            format_type: "audio/pcm".to_string(),
                            rate: params.sample_rate,
                        }),
                        transcription: Some(TranscriptionConfig {
                            model: model.to_string(),
                            language,
                        }),
                        turn_detection: Some(TurnDetection {
                            detection_type: VAD_DETECTION_TYPE.to_string(),
                            threshold: Some(VAD_THRESHOLD),
                            prefix_padding_ms: Some(VAD_PREFIX_PADDING_MS),
                            silence_duration_ms: Some(VAD_SILENCE_DURATION_MS),
                        }),
                    }),
                }),
                include: Some(vec!["item.input_audio_transcription.logprobs".to_string()]),
            },
        };

        let json = serde_json::to_string(&session_config).ok()?;
        tracing::debug!(
            hyprnote.payload.size_bytes = json.len() as u64,
            "openai_session_update_payload"
        );
        Some(Message::Text(json.into()))
    }

    fn finalize_message(&self) -> Message {
        let commit = InputAudioBufferCommit {
            event_type: "input_audio_buffer.commit".to_string(),
        };
        Message::Text(serde_json::to_string(&commit).unwrap().into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let event: OpenAIEvent = match serde_json::from_str(raw) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(
                    error = ?e,
                    hyprnote.payload.size_bytes = raw.len() as u64,
                    "openai_json_parse_failed"
                );
                return vec![];
            }
        };

        match event {
            OpenAIEvent::SessionCreated { session } => {
                tracing::debug!(
                    hyprnote.stt.provider_session.id = %session.id,
                    "openai_session_created"
                );
                vec![]
            }
            OpenAIEvent::SessionUpdated { session } => {
                tracing::debug!(
                    hyprnote.stt.provider_session.id = %session.id,
                    "openai_session_updated"
                );
                vec![]
            }
            OpenAIEvent::InputAudioBufferCommitted { item_id } => {
                tracing::debug!(hyprnote.stt.item.id = %item_id, "openai_audio_buffer_committed");
                vec![]
            }
            OpenAIEvent::InputAudioBufferCleared => {
                tracing::debug!("openai_audio_buffer_cleared");
                vec![]
            }
            OpenAIEvent::InputAudioBufferSpeechStarted { item_id } => {
                tracing::debug!(hyprnote.stt.item.id = %item_id, "openai_speech_started");
                vec![]
            }
            OpenAIEvent::InputAudioBufferSpeechStopped { item_id } => {
                tracing::debug!(hyprnote.stt.item.id = %item_id, "openai_speech_stopped");
                vec![]
            }
            OpenAIEvent::ConversationItemInputAudioTranscriptionCompleted {
                item_id,
                content_index,
                transcript,
            } => {
                tracing::debug!(
                    hyprnote.stt.item.id = %item_id,
                    hyprnote.stt.content_index = content_index,
                    hyprnote.transcript.char_count = transcript.chars().count() as u64,
                    "openai_transcription_completed"
                );
                Self::build_transcript_response(&transcript, true, true)
            }
            OpenAIEvent::ConversationItemInputAudioTranscriptionDelta {
                item_id,
                content_index,
                delta,
            } => {
                tracing::debug!(
                    hyprnote.stt.item.id = %item_id,
                    hyprnote.stt.content_index = content_index,
                    hyprnote.transcript.char_count = delta.chars().count() as u64,
                    "openai_transcription_delta"
                );
                Self::build_transcript_response(&delta, false, false)
            }
            OpenAIEvent::ConversationItemInputAudioTranscriptionFailed {
                item_id, error, ..
            } => {
                tracing::error!(
                    hyprnote.stt.item.id = %item_id,
                    error.type = %error.error_type,
                    error = %error.message,
                    "openai_transcription_failed"
                );
                vec![StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: format!("{}: {}", error.error_type, error.message),
                    provider: "openai".to_string(),
                }]
            }
            OpenAIEvent::Error { error } => {
                tracing::error!(
                    error.type = %error.error_type,
                    error = %error.message,
                    "openai_error"
                );
                vec![StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: format!("{}: {}", error.error_type, error.message),
                    provider: "openai".to_string(),
                }]
            }
            OpenAIEvent::Unknown => {
                tracing::debug!(
                    hyprnote.payload.size_bytes = raw.len() as u64,
                    "openai_unknown_event"
                );
                vec![]
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct SessionUpdateEvent {
    #[serde(rename = "type")]
    event_type: String,
    session: SessionConfig,
}

#[derive(Debug, Serialize)]
struct SessionConfig {
    #[serde(rename = "type")]
    session_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio: Option<AudioConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct AudioConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<AudioInputConfig>,
}

#[derive(Debug, Serialize)]
struct AudioInputConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<AudioFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transcription: Option<TranscriptionConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn_detection: Option<TurnDetection>,
}

#[derive(Debug, Serialize)]
struct AudioFormat {
    #[serde(rename = "type")]
    format_type: String,
    rate: u32,
}

#[derive(Debug, Serialize)]
struct TranscriptionConfig {
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
}

#[derive(Debug, Serialize)]
struct TurnDetection {
    #[serde(rename = "type")]
    detection_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix_padding_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    silence_duration_ms: Option<u32>,
}

#[derive(Debug, Serialize)]
struct InputAudioBufferAppend {
    #[serde(rename = "type")]
    event_type: String,
    audio: String,
}

#[derive(Debug, Serialize)]
struct InputAudioBufferCommit {
    #[serde(rename = "type")]
    event_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum OpenAIEvent {
    #[serde(rename = "session.created")]
    SessionCreated { session: SessionInfo },
    #[serde(rename = "session.updated")]
    SessionUpdated { session: SessionInfo },
    #[serde(rename = "input_audio_buffer.committed")]
    InputAudioBufferCommitted { item_id: String },
    #[serde(rename = "input_audio_buffer.cleared")]
    InputAudioBufferCleared,
    #[serde(rename = "input_audio_buffer.speech_started")]
    InputAudioBufferSpeechStarted { item_id: String },
    #[serde(rename = "input_audio_buffer.speech_stopped")]
    InputAudioBufferSpeechStopped { item_id: String },
    #[serde(rename = "conversation.item.input_audio_transcription.completed")]
    ConversationItemInputAudioTranscriptionCompleted {
        item_id: String,
        content_index: u32,
        transcript: String,
    },
    #[serde(rename = "conversation.item.input_audio_transcription.delta")]
    ConversationItemInputAudioTranscriptionDelta {
        item_id: String,
        content_index: u32,
        delta: String,
    },
    #[serde(rename = "conversation.item.input_audio_transcription.failed")]
    ConversationItemInputAudioTranscriptionFailed {
        item_id: String,
        content_index: u32,
        error: OpenAIError,
    },
    #[serde(rename = "error")]
    Error { error: OpenAIError },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct SessionInfo {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIError {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

impl OpenAIAdapter {
    fn build_transcript_response(
        transcript: &str,
        is_final: bool,
        speech_final: bool,
    ) -> Vec<StreamResponse> {
        if transcript.is_empty() {
            return vec![];
        }

        let words: Vec<_> = transcript
            .split_whitespace()
            .map(|word| WordBuilder::new(word).confidence(1.0).build())
            .collect();

        let (start, duration) = calculate_time_span(&words);

        let channel = Channel {
            alternatives: vec![Alternatives {
                transcript: transcript.to_string(),
                words,
                confidence: 1.0,
                languages: vec![],
            }],
        };

        vec![StreamResponse::TranscriptResponse {
            is_final,
            speech_final,
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

    use super::OpenAIAdapter;
    use crate::ListenClient;
    use crate::test_utils::{
        UrlTestCase, run_dual_test_with_rate, run_single_test_with_rate, run_url_test_cases,
    };

    const API_BASE: &str = "wss://api.openai.com";
    const OPENAI_SAMPLE_RATE: u32 = 24000;

    #[test]
    fn test_base_url() {
        run_url_test_cases(
            &OpenAIAdapter::default(),
            API_BASE,
            &[UrlTestCase {
                name: "base_url_structure",
                model: None,
                languages: &[ISO639::En],
                contains: &["api.openai.com"],
                not_contains: &[],
            }],
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_build_single() {
        let client = ListenClient::builder()
            .adapter::<OpenAIAdapter>()
            .api_base("wss://api.openai.com")
            .api_key(std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                model: Some("gpt-4o-transcribe".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                sample_rate: OPENAI_SAMPLE_RATE,
                ..Default::default()
            })
            .build_single()
            .await;

        run_single_test_with_rate(client, "openai", OPENAI_SAMPLE_RATE).await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_build_dual() {
        let client = ListenClient::builder()
            .adapter::<OpenAIAdapter>()
            .api_base("wss://api.openai.com")
            .api_key(std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                model: Some("gpt-4o-transcribe".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                sample_rate: OPENAI_SAMPLE_RATE,
                ..Default::default()
            })
            .build_dual()
            .await;

        run_dual_test_with_rate(client, "openai", OPENAI_SAMPLE_RATE).await;
    }
}

use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::{Deserialize, Serialize};

use super::DashScopeAdapter;
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::{WordBuilder, calculate_time_span};

const VAD_DETECTION_TYPE: &str = "server_vad";
const VAD_THRESHOLD: f32 = 0.5;
const VAD_PREFIX_PADDING_MS: u32 = 300;
const VAD_SILENCE_DURATION_MS: u32 = 500;
const DEFAULT_SAMPLE_RATE: u32 = 16000;

impl RealtimeSttAdapter for DashScopeAdapter {
    fn provider_name(&self) -> &'static str {
        "dashscope"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        DashScopeAdapter::is_supported_languages_live(languages)
    }

    fn supports_native_multichannel(&self) -> bool {
        false
    }

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, _channels: u8) -> url::Url {
        let (mut url, existing_params) = Self::build_ws_url_from_base(api_base);

        let default = crate::providers::Provider::DashScope.default_live_model();
        let model = match params.model.as_deref() {
            Some(m) if crate::providers::is_meta_model(m) => default,
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
        api_key.and_then(|k| crate::providers::Provider::DashScope.build_auth_header(k))
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

        let sample_rate = if params.sample_rate == 0 {
            DEFAULT_SAMPLE_RATE
        } else {
            params.sample_rate
        };

        let session_config = SessionUpdateEvent {
            event_type: "session.update".to_string(),
            session: SessionConfig {
                modalities: vec!["text".to_string()],
                transcription: Some(TranscriptionConfig {
                    language,
                    input_audio_format: "pcm".to_string(),
                    input_sample_rate: sample_rate,
                }),
                turn_detection: Some(TurnDetection {
                    detection_type: VAD_DETECTION_TYPE.to_string(),
                    threshold: Some(VAD_THRESHOLD),
                    prefix_padding_ms: Some(VAD_PREFIX_PADDING_MS),
                    silence_duration_ms: Some(VAD_SILENCE_DURATION_MS),
                }),
            },
        };

        let json = serde_json::to_string(&session_config).ok()?;
        tracing::debug!(hyprnote.payload.raw = %json, "dashscope_session_update_payload");
        Some(Message::Text(json.into()))
    }

    fn finalize_message(&self) -> Message {
        let finish = SessionFinish {
            event_type: "session.finish".to_string(),
        };
        Message::Text(serde_json::to_string(&finish).unwrap().into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let event: DashScopeEvent = match serde_json::from_str(raw) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(
                    error.message = ?e,
                    hyprnote.payload.raw = raw,
                    "dashscope_json_parse_failed"
                );
                return vec![];
            }
        };

        match event {
            DashScopeEvent::SessionCreated { session } => {
                tracing::debug!(
                    hyprnote.stt.provider_session.id = %session.id,
                    "dashscope_session_created"
                );
                vec![]
            }
            DashScopeEvent::SessionUpdated { session } => {
                tracing::debug!(
                    hyprnote.stt.provider_session.id = %session.id,
                    "dashscope_session_updated"
                );
                vec![]
            }
            DashScopeEvent::InputAudioBufferCommitted { item_id } => {
                tracing::debug!(
                    hyprnote.stt.item.id = %item_id,
                    "dashscope_audio_buffer_committed"
                );
                vec![]
            }
            DashScopeEvent::InputAudioBufferCleared => {
                tracing::debug!("dashscope_audio_buffer_cleared");
                vec![]
            }
            DashScopeEvent::InputAudioBufferSpeechStarted { item_id } => {
                tracing::debug!(hyprnote.stt.item.id = %item_id, "dashscope_speech_started");
                vec![]
            }
            DashScopeEvent::InputAudioBufferSpeechStopped { item_id } => {
                tracing::debug!(hyprnote.stt.item.id = %item_id, "dashscope_speech_stopped");
                vec![]
            }
            DashScopeEvent::ConversationItemInputAudioTranscriptionCompleted {
                item_id,
                transcript,
                ..
            } => {
                tracing::debug!(
                    hyprnote.stt.item.id = %item_id,
                    hyprnote.transcript.text = %transcript,
                    "dashscope_transcription_completed"
                );
                Self::build_transcript_response(&transcript, true, true)
            }
            DashScopeEvent::ConversationItemInputAudioTranscriptionText {
                item_id, text, ..
            } => {
                tracing::debug!(
                    hyprnote.stt.item.id = %item_id,
                    hyprnote.transcript.text = %text,
                    "dashscope_transcription_text"
                );
                Self::build_transcript_response(&text, false, false)
            }
            DashScopeEvent::ConversationItemInputAudioTranscriptionFailed {
                item_id,
                error,
                ..
            } => {
                tracing::error!(
                    hyprnote.stt.item.id = %item_id,
                    error.type = %error.error_type,
                    error.message = %error.message,
                    "dashscope_transcription_failed"
                );
                vec![StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: format!("{}: {}", error.error_type, error.message),
                    provider: "dashscope".to_string(),
                }]
            }
            DashScopeEvent::Error { error } => {
                tracing::error!(
                    error.type = %error.error_type,
                    error.message = %error.message,
                    "dashscope_error"
                );
                vec![StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: format!("{}: {}", error.error_type, error.message),
                    provider: "dashscope".to_string(),
                }]
            }
            DashScopeEvent::Unknown => {
                tracing::debug!(hyprnote.payload.raw = raw, "dashscope_unknown_event");
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
    modalities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transcription: Option<TranscriptionConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn_detection: Option<TurnDetection>,
}

#[derive(Debug, Serialize)]
struct TranscriptionConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    input_audio_format: String,
    input_sample_rate: u32,
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
#[allow(dead_code)]
struct InputAudioBufferCommit {
    #[serde(rename = "type")]
    event_type: String,
}

#[derive(Debug, Serialize)]
struct SessionFinish {
    #[serde(rename = "type")]
    event_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum DashScopeEvent {
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
        #[serde(default)]
        content_index: Option<u32>,
        transcript: String,
    },
    #[serde(rename = "conversation.item.input_audio_transcription.text")]
    ConversationItemInputAudioTranscriptionText {
        item_id: String,
        #[serde(default)]
        content_index: Option<u32>,
        text: String,
    },
    #[serde(rename = "conversation.item.input_audio_transcription.failed")]
    ConversationItemInputAudioTranscriptionFailed {
        item_id: String,
        #[serde(default)]
        content_index: Option<u32>,
        error: DashScopeError,
    },
    #[serde(rename = "error")]
    Error { error: DashScopeError },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct SessionInfo {
    id: String,
}

#[derive(Debug, Deserialize)]
struct DashScopeError {
    #[serde(rename = "type", default)]
    error_type: String,
    #[serde(default)]
    message: String,
}

impl DashScopeAdapter {
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

    use super::DashScopeAdapter;
    use crate::ListenClient;
    use crate::test_utils::{UrlTestCase, run_url_test_cases};

    const API_BASE: &str = "wss://dashscope-intl.aliyuncs.com";

    #[test]
    fn test_base_url() {
        run_url_test_cases(
            &DashScopeAdapter::default(),
            API_BASE,
            &[UrlTestCase {
                name: "base_url_structure",
                model: None,
                languages: &[ISO639::En],
                contains: &["dashscope-intl.aliyuncs.com"],
                not_contains: &[],
            }],
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_build_single() {
        let client = ListenClient::builder()
            .adapter::<DashScopeAdapter>()
            .api_base("wss://dashscope-intl.aliyuncs.com")
            .api_key(std::env::var("DASHSCOPE_API_KEY").expect("DASHSCOPE_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                model: Some("qwen3-asr-flash-realtime".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                sample_rate: 16000,
                ..Default::default()
            })
            .build_single()
            .await;

        crate::test_utils::run_single_test(client, "dashscope").await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_build_dual() {
        let client = ListenClient::builder()
            .adapter::<DashScopeAdapter>()
            .api_base("wss://dashscope-intl.aliyuncs.com")
            .api_key(std::env::var("DASHSCOPE_API_KEY").expect("DASHSCOPE_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                model: Some("qwen3-asr-flash-realtime".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                sample_rate: 16000,
                ..Default::default()
            })
            .build_dual()
            .await;

        crate::test_utils::run_dual_test(client, "dashscope").await;
    }
}

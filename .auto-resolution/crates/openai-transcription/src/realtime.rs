use crate::batch::{TranscriptionLogprob, TranscriptionUsage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct SessionUpdateEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(rename = "type")]
    pub event_type: ClientEventType,
    pub session: SessionConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionConfig {
    #[serde(rename = "type")]
    pub session_type: SessionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<SessionInclude>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<AudioInputConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioInputConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<AudioFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub noise_reduction: Option<NoiseReductionConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcription: Option<TranscriptionConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_detection: Option<TurnDetectionConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioFormat {
    #[serde(rename = "type")]
    pub format_type: AudioFormatType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NoiseReductionConfig {
    #[serde(rename = "type")]
    pub noise_reduction_type: NoiseReductionType,
}

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionConfig {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TurnDetectionConfig {
    #[serde(rename = "type")]
    pub detection_type: TurnDetectionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_response: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupt_response: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_timeout_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eagerness: Option<TurnDetectionEagerness>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_padding_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silence_duration_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputAudioBufferAppendEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(rename = "type")]
    pub event_type: ClientEventType,
    pub audio: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputAudioBufferCommitEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(rename = "type")]
    pub event_type: ClientEventType,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputAudioBufferClearEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(rename = "type")]
    pub event_type: ClientEventType,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum ClientEventType {
    #[serde(rename = "session.update")]
    SessionUpdate,
    #[serde(rename = "input_audio_buffer.append")]
    InputAudioBufferAppend,
    #[serde(rename = "input_audio_buffer.commit")]
    InputAudioBufferCommit,
    #[serde(rename = "input_audio_buffer.clear")]
    InputAudioBufferClear,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum SessionType {
    #[serde(rename = "transcription")]
    Transcription,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum AudioFormatType {
    #[serde(rename = "audio/pcm")]
    AudioPcm,
    #[serde(rename = "audio/pcmu")]
    AudioPcmu,
    #[serde(rename = "audio/pcma")]
    AudioPcma,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum NoiseReductionType {
    #[serde(rename = "near_field")]
    NearField,
    #[serde(rename = "far_field")]
    FarField,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum TurnDetectionType {
    #[serde(rename = "server_vad")]
    ServerVad,
    #[serde(rename = "semantic_vad")]
    SemanticVad,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum TurnDetectionEagerness {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "auto")]
    Auto,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum SessionInclude {
    #[serde(rename = "item.input_audio_transcription.logprobs")]
    InputAudioTranscriptionLogprobs,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ServerEvent {
    #[serde(rename = "session.created")]
    SessionCreated {
        event_id: String,
        session: SessionInfo,
    },
    #[serde(rename = "session.updated")]
    SessionUpdated {
        event_id: String,
        session: SessionInfo,
    },
    #[serde(rename = "input_audio_buffer.committed")]
    InputAudioBufferCommitted { event_id: String, item_id: String },
    #[serde(rename = "input_audio_buffer.cleared")]
    InputAudioBufferCleared { event_id: String },
    #[serde(rename = "input_audio_buffer.speech_started")]
    InputAudioBufferSpeechStarted {
        event_id: String,
        item_id: String,
        audio_start_ms: u64,
    },
    #[serde(rename = "input_audio_buffer.speech_stopped")]
    InputAudioBufferSpeechStopped {
        event_id: String,
        item_id: String,
        audio_end_ms: u64,
    },
    #[serde(rename = "input_audio_buffer.timeout_triggered")]
    InputAudioBufferTimeoutTriggered {
        event_id: String,
        item_id: String,
        audio_start_ms: u64,
        audio_end_ms: u64,
    },
    #[serde(rename = "conversation.item.input_audio_transcription.completed")]
    ConversationItemInputAudioTranscriptionCompleted {
        event_id: String,
        item_id: String,
        content_index: u32,
        transcript: String,
        usage: Option<TranscriptionUsage>,
        #[serde(default)]
        logprobs: Vec<TranscriptionLogprob>,
    },
    #[serde(rename = "conversation.item.input_audio_transcription.delta")]
    ConversationItemInputAudioTranscriptionDelta {
        event_id: String,
        item_id: String,
        content_index: Option<u32>,
        delta: String,
        #[serde(default)]
        logprobs: Vec<TranscriptionLogprob>,
        obfuscation: Option<String>,
    },
    #[serde(rename = "conversation.item.input_audio_transcription.segment")]
    ConversationItemInputAudioTranscriptionSegment {
        event_id: String,
        item_id: String,
        content_index: u32,
        id: String,
        start: f64,
        end: f64,
        text: String,
        speaker: Option<String>,
    },
    #[serde(rename = "conversation.item.input_audio_transcription.failed")]
    ConversationItemInputAudioTranscriptionFailed {
        event_id: String,
        item_id: String,
        content_index: u32,
        error: ErrorInfo,
    },
    #[serde(rename = "error")]
    Error { event_id: String, error: ErrorInfo },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionInfo {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorInfo {
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub code: Option<String>,
    pub message: Option<String>,
    pub param: Option<String>,
    pub event_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_update_serializes_expected_shape() {
        let json = serde_json::to_value(SessionUpdateEvent {
            event_id: Some("event-123".to_string()),
            event_type: ClientEventType::SessionUpdate,
            session: SessionConfig {
                session_type: SessionType::Transcription,
                audio: Some(AudioConfig {
                    input: Some(AudioInputConfig {
                        format: Some(AudioFormat {
                            format_type: AudioFormatType::AudioPcm,
                            rate: Some(24_000),
                        }),
                        noise_reduction: Some(NoiseReductionConfig {
                            noise_reduction_type: NoiseReductionType::NearField,
                        }),
                        transcription: Some(TranscriptionConfig {
                            model: "gpt-4o-transcribe".to_string(),
                            language: Some("en".to_string()),
                            prompt: Some("expect technical terms".to_string()),
                        }),
                        turn_detection: Some(TurnDetectionConfig {
                            detection_type: TurnDetectionType::ServerVad,
                            create_response: None,
                            interrupt_response: Some(true),
                            idle_timeout_ms: Some(5_000),
                            eagerness: None,
                            threshold: Some(0.5),
                            prefix_padding_ms: Some(300),
                            silence_duration_ms: Some(500),
                        }),
                    }),
                }),
                include: Some(vec![SessionInclude::InputAudioTranscriptionLogprobs]),
            },
        })
        .expect("serialize session");

        assert_eq!(json["type"], "session.update");
        assert_eq!(json["event_id"], "event-123");
        assert_eq!(json["session"]["type"], "transcription");
        assert_eq!(
            json["session"]["audio"]["input"]["format"]["type"],
            "audio/pcm"
        );
        assert_eq!(json["session"]["audio"]["input"]["format"]["rate"], 24_000);
        assert_eq!(
            json["session"]["audio"]["input"]["noise_reduction"]["type"],
            "near_field"
        );
        assert_eq!(
            json["session"]["audio"]["input"]["transcription"]["prompt"],
            "expect technical terms"
        );
        assert_eq!(
            json["session"]["audio"]["input"]["turn_detection"]["idle_timeout_ms"],
            5_000
        );
    }

    #[test]
    fn parses_completed_server_event() {
        let event: ServerEvent = serde_json::from_str(
            r#"{
                "type": "conversation.item.input_audio_transcription.completed",
                "event_id": "event-123",
                "item_id": "item-123",
                "content_index": 0,
                "transcript": "hello world",
                "usage": {
                    "type": "tokens",
                    "total_tokens": 22,
                    "input_tokens": 13,
                    "output_tokens": 9,
                    "input_token_details": {
                        "text_tokens": 0,
                        "audio_tokens": 13
                    }
                }
            }"#,
        )
        .expect("parse event");

        match event {
            ServerEvent::ConversationItemInputAudioTranscriptionCompleted {
                event_id,
                item_id,
                content_index,
                transcript,
                usage,
                ..
            } => {
                assert_eq!(event_id, "event-123");
                assert_eq!(item_id, "item-123");
                assert_eq!(content_index, 0);
                assert_eq!(transcript, "hello world");
                assert!(usage.is_some());
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn parses_delta_and_segment_server_events() {
        let delta: ServerEvent = serde_json::from_str(
            r#"{
                "type": "conversation.item.input_audio_transcription.delta",
                "event_id": "event-delta",
                "item_id": "item-123",
                "content_index": 0,
                "delta": "Hey",
                "logprobs": [
                    {
                        "token": "Hey",
                        "bytes": [72, 101, 121],
                        "logprob": -0.01
                    }
                ],
                "obfuscation": "abc123"
            }"#,
        )
        .expect("parse delta");

        match delta {
            ServerEvent::ConversationItemInputAudioTranscriptionDelta {
                event_id,
                item_id,
                content_index,
                delta,
                logprobs,
                obfuscation,
            } => {
                assert_eq!(event_id, "event-delta");
                assert_eq!(item_id, "item-123");
                assert_eq!(content_index, Some(0));
                assert_eq!(delta, "Hey");
                assert_eq!(logprobs.len(), 1);
                assert_eq!(obfuscation.as_deref(), Some("abc123"));
            }
            _ => panic!("unexpected delta variant"),
        }

        let segment: ServerEvent = serde_json::from_str(
            r#"{
                "type": "conversation.item.input_audio_transcription.segment",
                "event_id": "event-segment",
                "item_id": "item-123",
                "content_index": 0,
                "id": "seg-1",
                "start": 0.0,
                "end": 0.4,
                "text": "hello",
                "speaker": "spk_1"
            }"#,
        )
        .expect("parse segment");

        match segment {
            ServerEvent::ConversationItemInputAudioTranscriptionSegment {
                event_id,
                item_id,
                content_index,
                id,
                start,
                end,
                text,
                speaker,
            } => {
                assert_eq!(event_id, "event-segment");
                assert_eq!(item_id, "item-123");
                assert_eq!(content_index, 0);
                assert_eq!(id, "seg-1");
                assert_eq!(start, 0.0);
                assert_eq!(end, 0.4);
                assert_eq!(text, "hello");
                assert_eq!(speaker.as_deref(), Some("spk_1"));
            }
            _ => panic!("unexpected segment variant"),
        }
    }

    #[test]
    fn parses_timeout_triggered_server_event() {
        let event: ServerEvent = serde_json::from_str(
            r#"{
                "type": "input_audio_buffer.timeout_triggered",
                "event_id": "event-timeout",
                "audio_start_ms": 13216,
                "audio_end_ms": 19232,
                "item_id": "item-123"
            }"#,
        )
        .expect("parse timeout event");

        match event {
            ServerEvent::InputAudioBufferTimeoutTriggered {
                event_id,
                item_id,
                audio_start_ms,
                audio_end_ms,
            } => {
                assert_eq!(event_id, "event-timeout");
                assert_eq!(item_id, "item-123");
                assert_eq!(audio_start_ms, 13_216);
                assert_eq!(audio_end_ms, 19_232);
            }
            _ => panic!("unexpected timeout variant"),
        }
    }
}

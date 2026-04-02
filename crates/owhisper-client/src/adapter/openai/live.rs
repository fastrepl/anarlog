use hypr_ws_client::client::Message;
use openai_transcription::realtime::{
    AudioConfig, AudioFormat, AudioFormatType, AudioInputConfig, ClientEventType,
    InputAudioBufferAppendEvent, InputAudioBufferCommitEvent, ServerEvent, SessionConfig,
    SessionInclude, SessionType, SessionUpdateEvent, TranscriptionConfig, TurnDetectionConfig,
    TurnDetectionType,
};
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};

use super::OpenAIAdapter;
use crate::adapter::parsing::{WordBuilder, calculate_time_span};
use crate::adapter::{RealtimeSttAdapter, ResamplingMonoEncoder};

const VAD_THRESHOLD: f32 = 0.4;
const VAD_PREFIX_PADDING_MS: u32 = 300;
const VAD_SILENCE_DURATION_MS: u32 = 350;
pub(crate) const OPENAI_MIN_SAMPLE_RATE: u32 = 24_000;

impl RealtimeSttAdapter for OpenAIAdapter {
    type AudioEncoder = ResamplingMonoEncoder;

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

    fn realtime_target_sample_rate(&self, _input_sample_rate: u32, _channels: u8) -> u32 {
        OPENAI_MIN_SAMPLE_RATE
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|k| crate::providers::Provider::OpenAI.build_auth_header(k))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn create_audio_encoder(
        &self,
        input_sample_rate: u32,
        target_sample_rate: u32,
        params: &ListenParams,
        _channels: u8,
    ) -> Result<Self::AudioEncoder, crate::Error> {
        let _ = params;
        ResamplingMonoEncoder::new(input_sample_rate, target_sample_rate, serialize_audio_chunk)
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
            event_id: None,
            event_type: ClientEventType::SessionUpdate,
            session: SessionConfig {
                session_type: SessionType::Transcription,
                audio: Some(AudioConfig {
                    input: Some(AudioInputConfig {
                        format: Some(AudioFormat {
                            format_type: AudioFormatType::AudioPcm,
                            rate: Some(params.sample_rate),
                        }),
                        transcription: Some(TranscriptionConfig {
                            model: model.to_string(),
                            language,
                            prompt: None,
                        }),
                        turn_detection: Some(TurnDetectionConfig {
                            detection_type: TurnDetectionType::ServerVad,
                            create_response: None,
                            interrupt_response: None,
                            idle_timeout_ms: None,
                            eagerness: None,
                            threshold: Some(VAD_THRESHOLD),
                            prefix_padding_ms: Some(VAD_PREFIX_PADDING_MS),
                            silence_duration_ms: Some(VAD_SILENCE_DURATION_MS),
                        }),
                        noise_reduction: None,
                    }),
                }),
                include: Some(vec![SessionInclude::InputAudioTranscriptionLogprobs]),
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
        let commit = InputAudioBufferCommitEvent {
            event_id: None,
            event_type: ClientEventType::InputAudioBufferCommit,
        };
        Message::Text(serde_json::to_string(&commit).unwrap().into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let event: ServerEvent = match serde_json::from_str(raw) {
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
            ServerEvent::SessionCreated { session, .. } => {
                tracing::debug!(
                    hyprnote.stt.provider_session.id = %session.id,
                    "openai_session_created"
                );
                vec![]
            }
            ServerEvent::SessionUpdated { session, .. } => {
                tracing::debug!(
                    hyprnote.stt.provider_session.id = %session.id,
                    "openai_session_updated"
                );
                vec![]
            }
            ServerEvent::InputAudioBufferCommitted { item_id, .. } => {
                tracing::debug!(hyprnote.stt.item.id = %item_id, "openai_audio_buffer_committed");
                vec![]
            }
            ServerEvent::InputAudioBufferCleared { .. } => {
                tracing::debug!("openai_audio_buffer_cleared");
                vec![]
            }
            ServerEvent::InputAudioBufferSpeechStarted {
                item_id,
                audio_start_ms,
                ..
            } => {
                tracing::debug!(
                    hyprnote.stt.item.id = %item_id,
                    hyprnote.stt.audio_start_ms = audio_start_ms,
                    "openai_speech_started"
                );
                vec![]
            }
            ServerEvent::InputAudioBufferSpeechStopped {
                item_id,
                audio_end_ms,
                ..
            } => {
                tracing::debug!(
                    hyprnote.stt.item.id = %item_id,
                    hyprnote.stt.audio_end_ms = audio_end_ms,
                    "openai_speech_stopped"
                );
                vec![]
            }
            ServerEvent::InputAudioBufferTimeoutTriggered {
                item_id,
                audio_start_ms,
                audio_end_ms,
                ..
            } => {
                tracing::debug!(
                    hyprnote.stt.item.id = %item_id,
                    hyprnote.stt.audio_start_ms = audio_start_ms,
                    hyprnote.stt.audio_end_ms = audio_end_ms,
                    "openai_audio_buffer_timeout_triggered"
                );
                vec![]
            }
            ServerEvent::ConversationItemInputAudioTranscriptionCompleted {
                item_id,
                content_index,
                transcript,
                ..
            } => {
                tracing::debug!(
                    hyprnote.stt.item.id = %item_id,
                    hyprnote.stt.content_index = content_index,
                    hyprnote.transcript.char_count = transcript.chars().count() as u64,
                    "openai_transcription_completed"
                );
                Self::build_transcript_response(&transcript, true, true)
            }
            ServerEvent::ConversationItemInputAudioTranscriptionDelta {
                item_id,
                content_index,
                delta,
                obfuscation,
                ..
            } => {
                tracing::debug!(
                    hyprnote.stt.item.id = %item_id,
                    hyprnote.stt.content_index = content_index.unwrap_or_default(),
                    hyprnote.transcript.char_count = delta.chars().count() as u64,
                    "openai_transcription_delta"
                );
                if let Some(obfuscation) = obfuscation {
                    tracing::trace!(hyprnote.stt.obfuscation = %obfuscation);
                }
                Self::build_transcript_response(&delta, false, false)
            }
            ServerEvent::ConversationItemInputAudioTranscriptionFailed {
                item_id, error, ..
            } => {
                let error_type = error.error_type.as_deref().unwrap_or("unknown_error");
                let message = error.message.as_deref().unwrap_or("unknown error");
                tracing::error!(
                    hyprnote.stt.item.id = %item_id,
                    error.type = %error_type,
                    error = %message,
                    "openai_transcription_failed"
                );
                vec![StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: format!("{}: {}", error_type, message),
                    provider: "openai".to_string(),
                }]
            }
            ServerEvent::ConversationItemInputAudioTranscriptionSegment { .. } => vec![],
            ServerEvent::Error { error, .. } => {
                let error_type = error.error_type.as_deref().unwrap_or("unknown_error");
                let message = error.message.as_deref().unwrap_or("unknown error");
                tracing::error!(
                    error.type = %error_type,
                    error = %message,
                    "openai_error"
                );
                vec![StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: format!("{}: {}", error_type, message),
                    provider: "openai".to_string(),
                }]
            }
            ServerEvent::Unknown => {
                tracing::debug!(
                    hyprnote.payload.size_bytes = raw.len() as u64,
                    "openai_unknown_event"
                );
                vec![]
            }
        }
    }
}

fn serialize_audio_chunk(audio: bytes::Bytes) -> Result<Message, crate::Error> {
    use base64::Engine;
    let base64_audio = base64::engine::general_purpose::STANDARD.encode(&audio);
    let event = InputAudioBufferAppendEvent {
        event_id: None,
        event_type: ClientEventType::InputAudioBufferAppend,
        audio: base64_audio,
    };
    Ok(Message::Text(serde_json::to_string(&event).unwrap().into()))
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
    use base64::Engine;
    use hypr_language::ISO639;
    use hypr_ws_client::client::Message;

    use super::{OPENAI_MIN_SAMPLE_RATE, OpenAIAdapter};
    use crate::test_utils::{
        UrlTestCase, run_dual_test_with_rate, run_single_test_with_rate, run_url_test_cases,
    };
    use crate::{ListenClient, RealtimeAudioEncoder, RealtimeAudioInput, RealtimeSttAdapter};

    const API_BASE: &str = "wss://api.openai.com";
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

    #[test]
    fn target_rate_is_24k_for_non_24k_input() {
        let adapter = OpenAIAdapter::default();
        assert_eq!(
            adapter.realtime_target_sample_rate(16_000, 1),
            OPENAI_MIN_SAMPLE_RATE
        );
    }

    #[test]
    fn target_rate_stays_24k_for_24k_input() {
        let adapter = OpenAIAdapter::default();
        assert_eq!(
            adapter.realtime_target_sample_rate(24_000, 1),
            OPENAI_MIN_SAMPLE_RATE
        );
    }

    #[test]
    fn encoder_resamples_16k_input_to_openai_rate() {
        let adapter = OpenAIAdapter::default();
        let params = owhisper_interface::ListenParams {
            sample_rate: 24_000,
            ..Default::default()
        };
        let mut encoder = adapter
            .create_audio_encoder(16_000, OPENAI_MIN_SAMPLE_RATE, &params, 1)
            .unwrap();
        let push_output = encoder
            .push(RealtimeAudioInput::Mono(bytes::Bytes::from_static(&[0, 0])))
            .unwrap();
        assert!(push_output.is_empty());

        let flush_output = encoder.flush().unwrap();
        assert_eq!(flush_output.len(), 1);
        let Message::Text(json) = &flush_output[0] else {
            panic!("expected text message");
        };
        let payload: serde_json::Value = serde_json::from_str(json).unwrap();
        let encoded_audio = payload["audio"].as_str().unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded_audio)
            .unwrap();
        assert!(!decoded.is_empty());
        assert!(encoder.flush().unwrap().is_empty());
    }

    #[test]
    fn encoder_passthroughs_24k_input_without_buffering() {
        let adapter = OpenAIAdapter::default();
        let params = owhisper_interface::ListenParams {
            sample_rate: 24_000,
            ..Default::default()
        };
        let mut encoder = adapter
            .create_audio_encoder(24_000, OPENAI_MIN_SAMPLE_RATE, &params, 1)
            .unwrap();
        let push_output = encoder
            .push(RealtimeAudioInput::Mono(bytes::Bytes::from_static(&[0, 0])))
            .unwrap();
        assert_eq!(push_output.len(), 1);
        let Message::Text(json) = &push_output[0] else {
            panic!("expected text message");
        };
        let payload: serde_json::Value = serde_json::from_str(json).unwrap();
        let encoded_audio = payload["audio"].as_str().unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded_audio)
            .unwrap();
        assert_eq!(decoded, vec![0, 0]);
        assert!(encoder.flush().unwrap().is_empty());
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
                sample_rate: OPENAI_MIN_SAMPLE_RATE,
                ..Default::default()
            })
            .build_single()
            .await;

        run_single_test_with_rate(client, "openai", OPENAI_MIN_SAMPLE_RATE).await;
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
                sample_rate: OPENAI_MIN_SAMPLE_RATE,
                ..Default::default()
            })
            .build_dual()
            .await;

        run_dual_test_with_rate(client, "openai", OPENAI_MIN_SAMPLE_RATE).await;
    }
}

use anlg_ws_client::client::Message;
use openai_transcription::realtime::{
    AudioConfig, AudioFormat, AudioFormatType, AudioInputConfig, ClientEventType,
    InputAudioBufferAppendEvent, InputAudioBufferCommitEvent, ServerEvent, SessionConfig,
    SessionInclude, SessionType, SessionUpdateEvent, TranscriptionConfig, TurnDetectionConfig,
    TurnDetectionType,
};
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};

use super::{OpenAIAdapter, OpenAILiveItem, OpenAILiveState};
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::{WordBuilder, ms_to_secs};

const VAD_THRESHOLD: f32 = 0.4;
const VAD_PREFIX_PADDING_MS: u32 = 300;
const VAD_SILENCE_DURATION_MS: u32 = 350;
const MAX_LIVE_ITEMS: usize = 64;
const MAX_BLOCKED_COMPLETED_ITEMS: usize = 16;
const MAX_ITEM_ID_BYTES: usize = 1_024;
const MAX_ITEM_TRANSCRIPT_BYTES: usize = 64 * 1_024;
const MAX_RETAINED_TRANSCRIPT_BYTES: usize = 512 * 1_024;

type CompletedLiveItem = (String, u64, u64);

struct LiveStateReset {
    completed: Vec<CompletedLiveItem>,
    reason: &'static str,
}

impl RealtimeSttAdapter for OpenAIAdapter {
    fn fork_session(&self) -> Self {
        let input_sample_rate = self
            .live_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .input_sample_rate;
        Self::for_live_sample_rate(input_sample_rate)
    }

    fn provider_name(&self) -> &'static str {
        "openai"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
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
            url.query_pairs_mut().extend_pairs(&existing_params);
        }

        url
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|key| crate::providers::Provider::OpenAI.build_auth_header(key))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn audio_to_message(&self, audio: bytes::Bytes) -> Message {
        use base64::Engine;

        let input_sample_rate = self
            .live_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .input_sample_rate;
        let audio = resample_pcm16(
            &audio,
            input_sample_rate,
            crate::providers::Provider::OpenAI.default_live_sample_rate(),
        );
        let event = InputAudioBufferAppendEvent {
            event_id: None,
            event_type: ClientEventType::InputAudioBufferAppend,
            audio: base64::engine::general_purpose::STANDARD.encode(audio),
        };
        Message::Text(serde_json::to_string(&event).unwrap().into())
    }

    fn initial_message(
        &self,
        _api_key: Option<&str>,
        params: &ListenParams,
        _channels: u8,
    ) -> Option<Message> {
        let default = crate::providers::Provider::OpenAI.default_live_model();
        let model = match params.model.as_deref() {
            Some(model) if crate::providers::is_meta_model(model) => default,
            Some(model) => model,
            None => default,
        };
        let uses_plural_hints = matches!(model, "gpt-live-transcribe" | "gpt-transcribe");
        let language_codes = params
            .languages
            .iter()
            .map(|language| language.iso639().code().to_string())
            .collect::<Vec<_>>();
        let sample_rate = crate::providers::Provider::OpenAI.default_live_sample_rate();

        self.live_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .input_sample_rate = params.sample_rate;

        let event = SessionUpdateEvent {
            event_id: None,
            event_type: ClientEventType::SessionUpdate,
            session: SessionConfig {
                session_type: SessionType::Transcription,
                audio: Some(AudioConfig {
                    input: Some(AudioInputConfig {
                        format: Some(AudioFormat {
                            format_type: AudioFormatType::AudioPcm,
                            rate: Some(sample_rate),
                        }),
                        transcription: Some(TranscriptionConfig {
                            model: model.to_string(),
                            language: (!uses_plural_hints)
                                .then(|| language_codes.first().cloned())
                                .flatten(),
                            languages: (uses_plural_hints && !language_codes.is_empty())
                                .then_some(language_codes),
                            keywords: (uses_plural_hints && !params.keywords.is_empty())
                                .then(|| params.keywords.clone()),
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
                include: (!uses_plural_hints)
                    .then_some(vec![SessionInclude::InputAudioTranscriptionLogprobs]),
            },
        };

        let json = serde_json::to_string(&event).ok()?;
        tracing::debug!(
            anarlog.payload.size_bytes = json.len() as u64,
            "openai_session_update_payload"
        );
        Some(Message::Text(json.into()))
    }

    fn finalize_message(&self) -> Message {
        let event = InputAudioBufferCommitEvent {
            event_id: None,
            event_type: ClientEventType::InputAudioBufferCommit,
        };
        Message::Text(serde_json::to_string(&event).unwrap().into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let event: ServerEvent = match serde_json::from_str(raw) {
            Ok(event) => event,
            Err(error) => {
                tracing::warn!(
                    error = ?error,
                    anarlog.payload.size_bytes = raw.len() as u64,
                    "openai_json_parse_failed"
                );
                return vec![];
            }
        };

        match event {
            ServerEvent::SessionCreated { session, .. } => {
                tracing::debug!(
                    anarlog.stt.provider_session.id = %session.id,
                    "openai_session_created"
                );
                vec![]
            }
            ServerEvent::SessionUpdated { session, .. } => {
                tracing::debug!(
                    anarlog.stt.provider_session.id = %session.id,
                    "openai_session_updated"
                );
                vec![]
            }
            ServerEvent::InputAudioBufferCommitted { item_id, .. } => {
                tracing::debug!(anarlog.stt.item.id = %item_id, "openai_audio_buffer_committed");
                Self::finish_state_update(self.ensure_item(&item_id))
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
                    anarlog.stt.item.id = %item_id,
                    anarlog.stt.audio_start_ms = audio_start_ms,
                    "openai_speech_started"
                );
                Self::finish_state_update(self.record_item_start(&item_id, audio_start_ms))
            }
            ServerEvent::InputAudioBufferSpeechStopped {
                item_id,
                audio_end_ms,
                ..
            } => {
                tracing::debug!(
                    anarlog.stt.item.id = %item_id,
                    anarlog.stt.audio_end_ms = audio_end_ms,
                    "openai_speech_stopped"
                );
                Self::finish_state_update(self.record_item_end(&item_id, audio_end_ms))
            }
            ServerEvent::InputAudioBufferTimeoutTriggered {
                item_id,
                audio_start_ms,
                audio_end_ms,
                ..
            } => {
                tracing::debug!(
                    anarlog.stt.item.id = %item_id,
                    anarlog.stt.audio_start_ms = audio_start_ms,
                    anarlog.stt.audio_end_ms = audio_end_ms,
                    "openai_audio_buffer_timeout_triggered"
                );
                Self::finish_state_update(self.record_item_span(
                    &item_id,
                    audio_start_ms,
                    audio_end_ms,
                ))
            }
            ServerEvent::ConversationItemInputAudioTranscriptionCompleted {
                item_id,
                content_index,
                transcript,
                ..
            } => {
                tracing::debug!(
                    anarlog.stt.item.id = %item_id,
                    anarlog.stt.content_index = content_index,
                    anarlog.transcript.char_count = transcript.chars().count() as u64,
                    "openai_transcription_completed"
                );
                match self.complete_items(&item_id, &transcript) {
                    Ok(completed) => Self::completed_responses(completed),
                    Err(reset) => Self::reset_responses(reset),
                }
            }
            ServerEvent::ConversationItemInputAudioTranscriptionDelta {
                item_id,
                content_index,
                delta,
                obfuscation,
                ..
            } => {
                tracing::debug!(
                    anarlog.stt.item.id = %item_id,
                    anarlog.stt.content_index = content_index.unwrap_or_default(),
                    anarlog.transcript.char_count = delta.chars().count() as u64,
                    "openai_transcription_delta"
                );
                if let Some(obfuscation) = obfuscation {
                    tracing::trace!(anarlog.stt.obfuscation = %obfuscation);
                }
                match self.append_item_delta(&item_id, &delta) {
                    Ok((transcript, start_ms, end_ms)) => {
                        Self::build_transcript_response(&transcript, false, false, start_ms, end_ms)
                    }
                    Err(reset) => Self::reset_responses(reset),
                }
            }
            ServerEvent::ConversationItemInputAudioTranscriptionFailed {
                item_id, error, ..
            } => {
                let completed = self.discard_item(&item_id);
                let error_type = error.error_type.as_deref().unwrap_or("unknown_error");
                let message = error.message.as_deref().unwrap_or("unknown error");
                tracing::error!(
                    anarlog.stt.item.id = %item_id,
                    error.type = %error_type,
                    error = %message,
                    "openai_transcription_failed"
                );
                completed
                    .into_iter()
                    .flat_map(|(transcript, start_ms, end_ms)| {
                        Self::build_transcript_response(&transcript, true, true, start_ms, end_ms)
                    })
                    .collect()
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
                    error_message: format!("{error_type}: {message}"),
                    provider: "openai".to_string(),
                }]
            }
            ServerEvent::Unknown => {
                tracing::debug!(
                    anarlog.payload.size_bytes = raw.len() as u64,
                    "openai_unknown_event"
                );
                vec![]
            }
        }
    }
}

impl OpenAIAdapter {
    fn prepare_item(state: &mut OpenAILiveState, item_id: &str) -> Result<(), LiveStateReset> {
        if state.items.contains_key(item_id) {
            return Ok(());
        }
        if item_id.len() > MAX_ITEM_ID_BYTES {
            return Err(Self::reset_live_state(state, "item ID exceeded 1 KiB"));
        }
        if state.items.len() >= MAX_LIVE_ITEMS {
            return Err(Self::reset_live_state(
                state,
                "more than 64 transcription items remained pending",
            ));
        }

        state.item_order.push_back(item_id.to_string());
        state
            .items
            .insert(item_id.to_string(), OpenAILiveItem::default());
        Ok(())
    }

    fn ensure_item(&self, item_id: &str) -> Result<(), LiveStateReset> {
        let mut state = self
            .live_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::prepare_item(&mut state, item_id)
    }

    fn record_item_start(&self, item_id: &str, start_ms: u64) -> Result<(), LiveStateReset> {
        let mut state = self
            .live_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::prepare_item(&mut state, item_id)?;
        state.items.get_mut(item_id).unwrap().start_ms = Some(start_ms);
        Ok(())
    }

    fn record_item_end(&self, item_id: &str, end_ms: u64) -> Result<(), LiveStateReset> {
        let mut state = self
            .live_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::prepare_item(&mut state, item_id)?;
        state.items.get_mut(item_id).unwrap().end_ms = Some(end_ms);
        Ok(())
    }

    fn record_item_span(
        &self,
        item_id: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<(), LiveStateReset> {
        let mut state = self
            .live_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::prepare_item(&mut state, item_id)?;
        let item = state.items.get_mut(item_id).unwrap();
        item.start_ms = Some(start_ms);
        item.end_ms = Some(end_ms);
        Ok(())
    }

    fn append_item_delta(
        &self,
        item_id: &str,
        delta: &str,
    ) -> Result<(String, u64, u64), LiveStateReset> {
        let mut state = self
            .live_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::prepare_item(&mut state, item_id)?;

        let item_bytes = state.items.get(item_id).unwrap().transcript.len();
        let retained_bytes = Self::retained_transcript_bytes(&state);
        if delta.len() > MAX_ITEM_TRANSCRIPT_BYTES.saturating_sub(item_bytes) {
            return Err(Self::reset_live_state(
                &mut state,
                "one transcription item exceeded 64 KiB",
            ));
        }
        if delta.len() > MAX_RETAINED_TRANSCRIPT_BYTES.saturating_sub(retained_bytes) {
            return Err(Self::reset_live_state(
                &mut state,
                "retained transcription text exceeded 512 KiB",
            ));
        }

        let fallback_start_ms = state.fallback_end_ms;
        let item = state.items.get_mut(item_id).unwrap();
        item.transcript.push_str(delta);

        let transcript = item.transcript.clone();
        let start_ms = item.start_ms.unwrap_or(fallback_start_ms);
        let end_ms = item
            .end_ms
            .unwrap_or_else(|| synthetic_end_ms(start_ms, &transcript));
        Ok((transcript, start_ms, end_ms))
    }

    fn complete_items(
        &self,
        item_id: &str,
        transcript: &str,
    ) -> Result<Vec<CompletedLiveItem>, LiveStateReset> {
        let mut state = self
            .live_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::prepare_item(&mut state, item_id)?;

        let item = state.items.get(item_id).unwrap();
        let retained_without_item = Self::retained_transcript_bytes(&state)
            .saturating_sub(item.transcript.len())
            .saturating_sub(item.completed_transcript.as_ref().map_or(0, String::len));
        if transcript.len() > MAX_ITEM_TRANSCRIPT_BYTES {
            return Err(Self::reset_live_state(
                &mut state,
                "one completed transcription item exceeded 64 KiB",
            ));
        }
        if transcript.len() > MAX_RETAINED_TRANSCRIPT_BYTES.saturating_sub(retained_without_item) {
            return Err(Self::reset_live_state(
                &mut state,
                "retained transcription text exceeded 512 KiB",
            ));
        }

        let item = state.items.get_mut(item_id).unwrap();
        item.transcript = String::new();
        item.completed_transcript = Some(transcript.to_string());

        if Self::blocked_completed_items(&state) >= MAX_BLOCKED_COMPLETED_ITEMS {
            return Err(Self::reset_live_state(
                &mut state,
                "16 completed items were blocked behind an unfinished item",
            ));
        }

        Ok(Self::drain_completed(&mut state))
    }

    fn drain_completed(state: &mut OpenAILiveState) -> Vec<CompletedLiveItem> {
        let mut completed = Vec::new();
        loop {
            let Some(next_item_id) = state.item_order.front() else {
                break;
            };
            if state
                .items
                .get(next_item_id)
                .and_then(|item| item.completed_transcript.as_ref())
                .is_none()
            {
                break;
            }

            let next_item_id = state.item_order.pop_front().unwrap();
            let mut item = state.items.remove(&next_item_id).unwrap();
            let transcript = item.completed_transcript.take().unwrap();
            completed.push(Self::finish_item(state, item, transcript));
        }

        completed
    }

    fn finish_item(
        state: &mut OpenAILiveState,
        item: OpenAILiveItem,
        transcript: String,
    ) -> CompletedLiveItem {
        let observed_start_ms = item.start_ms.unwrap_or(state.fallback_end_ms);
        let start_ms = observed_start_ms.max(state.fallback_end_ms);
        let observed_duration_ms = item
            .end_ms
            .unwrap_or_else(|| synthetic_end_ms(observed_start_ms, &transcript))
            .saturating_sub(observed_start_ms);
        let end_ms = (start_ms + observed_duration_ms).max(synthetic_end_ms(start_ms, &transcript));
        state.fallback_end_ms = end_ms;
        (transcript, start_ms, end_ms)
    }

    fn retained_transcript_bytes(state: &OpenAILiveState) -> usize {
        state
            .items
            .values()
            .map(|item| {
                item.transcript.len() + item.completed_transcript.as_ref().map_or(0, String::len)
            })
            .sum()
    }

    fn blocked_completed_items(state: &OpenAILiveState) -> usize {
        let Some(head_id) = state.item_order.front() else {
            return 0;
        };
        if state
            .items
            .get(head_id)
            .and_then(|item| item.completed_transcript.as_ref())
            .is_some()
        {
            return 0;
        }

        state
            .item_order
            .iter()
            .skip(1)
            .filter(|item_id| {
                state
                    .items
                    .get(*item_id)
                    .and_then(|item| item.completed_transcript.as_ref())
                    .is_some()
            })
            .count()
    }

    fn reset_live_state(state: &mut OpenAILiveState, reason: &'static str) -> LiveStateReset {
        let mut completed = Vec::new();
        while let Some(item_id) = state.item_order.pop_front() {
            let Some(mut item) = state.items.remove(&item_id) else {
                continue;
            };
            let transcript = item.completed_transcript.take().or_else(|| {
                (!item.transcript.trim().is_empty()).then(|| std::mem::take(&mut item.transcript))
            });
            if let Some(transcript) = transcript {
                completed.push(Self::finish_item(state, item, transcript));
            }
        }
        state.items.clear();

        LiveStateReset { completed, reason }
    }

    fn finish_state_update(result: Result<(), LiveStateReset>) -> Vec<StreamResponse> {
        match result {
            Ok(()) => Vec::new(),
            Err(reset) => Self::reset_responses(reset),
        }
    }

    fn completed_responses(completed: Vec<CompletedLiveItem>) -> Vec<StreamResponse> {
        completed
            .into_iter()
            .flat_map(|(transcript, start_ms, end_ms)| {
                Self::build_transcript_response(&transcript, true, true, start_ms, end_ms)
            })
            .collect()
    }

    fn reset_responses(reset: LiveStateReset) -> Vec<StreamResponse> {
        tracing::warn!(reason = reset.reason, "openai_live_state_limit_exceeded");
        let mut responses = Self::completed_responses(reset.completed);
        responses.push(StreamResponse::ErrorResponse {
            error_code: None,
            error_message: format!(
                "OpenAI realtime state limit exceeded: {}; reconnecting to preserve transcript order",
                reset.reason
            ),
            provider: "openai".to_string(),
        });
        responses
    }

    fn discard_item(&self, item_id: &str) -> Vec<(String, u64, u64)> {
        let mut state = self
            .live_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.items.remove(item_id);
        state.item_order.retain(|queued| queued != item_id);
        Self::drain_completed(&mut state)
    }

    fn build_transcript_response(
        transcript: &str,
        is_final: bool,
        speech_final: bool,
        start_ms: u64,
        end_ms: u64,
    ) -> Vec<StreamResponse> {
        if transcript.trim().is_empty() {
            return vec![];
        }

        let raw_words = transcript.split_whitespace().collect::<Vec<_>>();
        let word_count = raw_words.len() as u64;
        let span_ms = end_ms.saturating_sub(start_ms).max(word_count);
        let words = raw_words
            .into_iter()
            .enumerate()
            .map(|(index, word)| {
                let index = index as u64;
                WordBuilder::new(word)
                    .start(ms_to_secs(start_ms + span_ms * index / word_count))
                    .end(ms_to_secs(start_ms + span_ms * (index + 1) / word_count))
                    .confidence(1.0)
                    .build()
            })
            .collect::<Vec<_>>();
        let start = ms_to_secs(start_ms);
        let duration = ms_to_secs(span_ms);

        vec![StreamResponse::TranscriptResponse {
            is_final,
            speech_final,
            from_finalize: false,
            start,
            duration,
            channel: Channel {
                alternatives: vec![Alternatives {
                    transcript: transcript.to_string(),
                    words,
                    confidence: 1.0,
                    languages: vec![],
                }],
            },
            metadata: Metadata::default(),
            channel_index: vec![0, 1],
        }]
    }
}

fn synthetic_end_ms(start_ms: u64, transcript: &str) -> u64 {
    start_ms + transcript.split_whitespace().count().max(1) as u64
}

fn resample_pcm16(audio: &[u8], input_rate: u32, output_rate: u32) -> Vec<u8> {
    if input_rate == 0 || input_rate == output_rate {
        return audio.to_vec();
    }

    let samples = audio
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    if samples.is_empty() {
        return vec![];
    }

    let output_len = ((samples.len() as u64 * output_rate as u64 + input_rate as u64 / 2)
        / input_rate as u64) as usize;
    let mut output = Vec::with_capacity(output_len * 2);

    for output_index in 0..output_len {
        let position = output_index as u64 * input_rate as u64;
        let left_index = ((position / output_rate as u64) as usize).min(samples.len() - 1);
        let right_index = (left_index + 1).min(samples.len() - 1);
        let remainder = (position % output_rate as u64) as i64;
        let denominator = output_rate as i64;
        let left = samples[left_index] as i64;
        let right = samples[right_index] as i64;
        let sample = (left * (denominator - remainder) + right * remainder) / denominator;
        output.extend_from_slice(&(sample as i16).to_le_bytes());
    }

    output
}

#[cfg(test)]
mod tests {
    use anlg_ws_client::client::Message;
    use base64::Engine;

    use super::*;

    fn initial_message_json(params: &ListenParams) -> serde_json::Value {
        match OpenAIAdapter::default()
            .initial_message(None, params, 1)
            .expect("initial message")
        {
            Message::Text(text) => serde_json::from_str(&text).expect("valid JSON"),
            _ => panic!("expected text message"),
        }
    }

    #[test]
    fn live_model_uses_plural_language_and_keyword_hints() {
        let json = initial_message_json(&ListenParams {
            languages: vec![
                anlg_language::ISO639::En.into(),
                anlg_language::ISO639::Ko.into(),
            ],
            keywords: vec!["technical term".to_string()],
            ..Default::default()
        });
        let transcription = &json["session"]["audio"]["input"]["transcription"];

        assert_eq!(transcription["model"], "gpt-live-transcribe");
        assert_eq!(transcription["languages"], serde_json::json!(["en", "ko"]));
        assert_eq!(
            transcription["keywords"],
            serde_json::json!(["technical term"])
        );
        assert!(transcription.get("language").is_none());
        assert!(json["session"].get("include").is_none());
    }

    #[test]
    fn legacy_model_uses_singular_language_hint() {
        let json = initial_message_json(&ListenParams {
            model: Some("gpt-4o-transcribe".to_string()),
            languages: vec![
                anlg_language::ISO639::En.into(),
                anlg_language::ISO639::Ko.into(),
            ],
            ..Default::default()
        });
        let transcription = &json["session"]["audio"]["input"]["transcription"];

        assert_eq!(transcription["language"], "en");
        assert!(transcription.get("languages").is_none());
        assert_eq!(
            json["session"]["include"],
            serde_json::json!(["item.input_audio_transcription.logprobs"])
        );
    }

    #[test]
    fn realtime_audio_is_declared_and_resampled_at_24khz() {
        let adapter = OpenAIAdapter::default();
        let params = ListenParams {
            sample_rate: 16_000,
            ..Default::default()
        };
        let message = adapter.initial_message(None, &params, 1).unwrap();
        let Message::Text(message) = message else {
            panic!("expected text message");
        };
        let json: serde_json::Value = serde_json::from_str(&message).unwrap();
        assert_eq!(json["session"]["audio"]["input"]["format"]["rate"], 24_000);

        let input = (0..160_i16).flat_map(i16::to_le_bytes).collect::<Vec<_>>();
        let Message::Text(message) = adapter.audio_to_message(input.into()) else {
            panic!("expected text message");
        };
        let json: serde_json::Value = serde_json::from_str(&message).unwrap();
        let output = base64::engine::general_purpose::STANDARD
            .decode(json["audio"].as_str().unwrap())
            .unwrap();
        assert_eq!(output.len(), 240 * std::mem::size_of::<i16>());
    }

    #[test]
    fn transcript_deltas_are_cumulative_and_final_turns_keep_monotonic_times() {
        let adapter = OpenAIAdapter::default();
        adapter.parse_response(
            r#"{"type":"input_audio_buffer.speech_started","event_id":"e1","item_id":"item-1","audio_start_ms":0}"#,
        );
        let first_delta = adapter.parse_response(
            r#"{"type":"conversation.item.input_audio_transcription.delta","event_id":"e2","item_id":"item-1","content_index":0,"delta":"Hello","logprobs":[]}"#,
        );
        let second_delta = adapter.parse_response(
            r#"{"type":"conversation.item.input_audio_transcription.delta","event_id":"e3","item_id":"item-1","content_index":0,"delta":" world","logprobs":[]}"#,
        );

        assert_eq!(transcript(&first_delta), "Hello");
        assert_eq!(transcript(&second_delta), "Hello world");

        adapter.parse_response(
            r#"{"type":"input_audio_buffer.speech_stopped","event_id":"e4","item_id":"item-1","audio_end_ms":1000}"#,
        );
        adapter.parse_response(
            r#"{"type":"input_audio_buffer.speech_started","event_id":"e5","item_id":"item-2","audio_start_ms":1200}"#,
        );
        adapter.parse_response(
            r#"{"type":"input_audio_buffer.speech_stopped","event_id":"e6","item_id":"item-2","audio_end_ms":1800}"#,
        );
        let out_of_order_final = adapter.parse_response(
            r#"{"type":"conversation.item.input_audio_transcription.completed","event_id":"e7","item_id":"item-2","content_index":0,"transcript":"Next turn","logprobs":[]}"#,
        );
        assert!(out_of_order_final.is_empty());
        let ordered_finals = adapter.parse_response(
            r#"{"type":"conversation.item.input_audio_transcription.completed","event_id":"e8","item_id":"item-1","content_index":0,"transcript":"Hello world","logprobs":[]}"#,
        );

        assert_eq!(transcript_at(&ordered_finals, 0), "Hello world");
        assert_eq!(transcript_at(&ordered_finals, 1), "Next turn");
        assert!(last_word_end_at(&ordered_finals, 0) <= first_word_start_at(&ordered_finals, 1));
    }

    #[test]
    fn forked_sessions_keep_transcript_state_isolated() {
        let adapter = OpenAIAdapter::for_live_sample_rate(16_000);
        let mic = adapter.fork_session();
        let speaker = adapter.fork_session();

        mic.parse_response(
            r#"{"type":"input_audio_buffer.speech_started","event_id":"e1","item_id":"item-1","audio_start_ms":0}"#,
        );
        speaker.parse_response(
            r#"{"type":"input_audio_buffer.speech_started","event_id":"e2","item_id":"item-1","audio_start_ms":0}"#,
        );
        let mic_delta = mic.parse_response(
            r#"{"type":"conversation.item.input_audio_transcription.delta","event_id":"e3","item_id":"item-1","content_index":0,"delta":"Mic","logprobs":[]}"#,
        );
        let speaker_delta = speaker.parse_response(
            r#"{"type":"conversation.item.input_audio_transcription.delta","event_id":"e4","item_id":"item-1","content_index":0,"delta":"Speaker","logprobs":[]}"#,
        );

        assert_eq!(transcript(&mic_delta), "Mic");
        assert_eq!(transcript(&speaker_delta), "Speaker");
    }

    #[test]
    fn failed_item_is_nonfatal_and_releases_completed_later_items() {
        let adapter = OpenAIAdapter::default();
        adapter.parse_response(
            r#"{"type":"input_audio_buffer.speech_started","event_id":"e1","item_id":"item-1","audio_start_ms":0}"#,
        );
        adapter.parse_response(
            r#"{"type":"input_audio_buffer.speech_started","event_id":"e2","item_id":"item-2","audio_start_ms":1000}"#,
        );
        let held_final = adapter.parse_response(
            r#"{"type":"conversation.item.input_audio_transcription.completed","event_id":"e3","item_id":"item-2","content_index":0,"transcript":"Later turn","logprobs":[]}"#,
        );
        assert!(held_final.is_empty());

        let responses = adapter.parse_response(
            r#"{"type":"conversation.item.input_audio_transcription.failed","event_id":"e4","item_id":"item-1","content_index":0,"error":{"type":"server_error","code":"server_error","message":"failed"}}"#,
        );

        assert_eq!(responses.len(), 1);
        assert_eq!(transcript(&responses), "Later turn");
    }

    #[test]
    fn stalled_head_releases_bounded_completed_backlog_before_reconnect() {
        let adapter = OpenAIAdapter::default();
        adapter.parse_response(
            r#"{"type":"input_audio_buffer.speech_started","event_id":"head-start","item_id":"head","audio_start_ms":0}"#,
        );
        adapter.parse_response(
            r#"{"type":"conversation.item.input_audio_transcription.delta","event_id":"head-delta","item_id":"head","content_index":0,"delta":"Head partial","logprobs":[]}"#,
        );

        let mut recovered = Vec::new();
        for index in 0..MAX_BLOCKED_COMPLETED_ITEMS {
            let item_id = format!("later-{index}");
            let started = serde_json::json!({
                "type": "input_audio_buffer.speech_started",
                "event_id": format!("start-{index}"),
                "item_id": item_id,
                "audio_start_ms": (index as u64 + 1) * 1_000,
            });
            assert!(adapter.parse_response(&started.to_string()).is_empty());

            let completed = serde_json::json!({
                "type": "conversation.item.input_audio_transcription.completed",
                "event_id": format!("done-{index}"),
                "item_id": item_id,
                "content_index": 0,
                "transcript": format!("Later {index}"),
                "logprobs": [],
            });
            recovered = adapter.parse_response(&completed.to_string());
            if index + 1 < MAX_BLOCKED_COMPLETED_ITEMS {
                assert!(recovered.is_empty());
            }
        }

        assert_eq!(
            recovered.len(),
            MAX_BLOCKED_COMPLETED_ITEMS + 2,
            "best-known head, completed backlog, then terminal recovery error"
        );
        assert_eq!(transcript_at(&recovered, 0), "Head partial");
        for index in 0..MAX_BLOCKED_COMPLETED_ITEMS {
            assert_eq!(
                transcript_at(&recovered, index + 1),
                format!("Later {index}")
            );
        }
        assert!(is_state_limit_error(recovered.last().unwrap()));

        let state = adapter.live_state.lock().unwrap();
        assert!(state.items.is_empty());
        assert!(state.item_order.is_empty());
    }

    #[test]
    fn cumulative_deltas_are_bounded_and_recovered_before_terminal_error() {
        let adapter = OpenAIAdapter::default();
        adapter.parse_response(
            r#"{"type":"input_audio_buffer.speech_started","event_id":"start","item_id":"item-1","audio_start_ms":0}"#,
        );
        let chunk = "x".repeat(1_024);
        for index in 0..(MAX_ITEM_TRANSCRIPT_BYTES / chunk.len()) {
            let delta = serde_json::json!({
                "type": "conversation.item.input_audio_transcription.delta",
                "event_id": format!("delta-{index}"),
                "item_id": "item-1",
                "content_index": 0,
                "delta": chunk,
                "logprobs": [],
            });
            let responses = adapter.parse_response(&delta.to_string());
            assert_eq!(responses.len(), 1);
        }
        let overflow = serde_json::json!({
            "type": "conversation.item.input_audio_transcription.delta",
            "event_id": "overflow",
            "item_id": "item-1",
            "content_index": 0,
            "delta": "y",
            "logprobs": [],
        });

        let responses = adapter.parse_response(&overflow.to_string());

        assert_eq!(responses.len(), 2);
        assert_eq!(transcript(&responses).len(), MAX_ITEM_TRANSCRIPT_BYTES);
        assert!(is_state_limit_error(&responses[1]));
        let state = adapter.live_state.lock().unwrap();
        assert!(state.items.is_empty());
        assert!(state.item_order.is_empty());
    }

    #[test]
    fn pending_item_count_is_bounded_and_fails_explicitly() {
        let adapter = OpenAIAdapter::default();
        for index in 0..MAX_LIVE_ITEMS {
            let started = serde_json::json!({
                "type": "input_audio_buffer.speech_started",
                "event_id": format!("start-{index}"),
                "item_id": format!("item-{index}"),
                "audio_start_ms": index as u64,
            });
            assert!(adapter.parse_response(&started.to_string()).is_empty());
        }

        let overflow = serde_json::json!({
            "type": "input_audio_buffer.speech_started",
            "event_id": "overflow",
            "item_id": "item-overflow",
            "audio_start_ms": MAX_LIVE_ITEMS as u64,
        });
        let responses = adapter.parse_response(&overflow.to_string());

        assert_eq!(responses.len(), 1);
        assert!(is_state_limit_error(&responses[0]));
        let state = adapter.live_state.lock().unwrap();
        assert!(state.items.is_empty());
        assert!(state.item_order.is_empty());
    }

    fn transcript(responses: &[StreamResponse]) -> &str {
        transcript_at(responses, 0)
    }

    fn transcript_at(responses: &[StreamResponse], index: usize) -> &str {
        let StreamResponse::TranscriptResponse { channel, .. } = &responses[index] else {
            panic!("expected transcript response");
        };
        &channel.alternatives[0].transcript
    }

    fn first_word_start_at(responses: &[StreamResponse], index: usize) -> f64 {
        let StreamResponse::TranscriptResponse { channel, .. } = &responses[index] else {
            panic!("expected transcript response");
        };
        channel.alternatives[0].words[0].start
    }

    fn last_word_end_at(responses: &[StreamResponse], index: usize) -> f64 {
        let StreamResponse::TranscriptResponse { channel, .. } = &responses[index] else {
            panic!("expected transcript response");
        };
        channel.alternatives[0].words.last().unwrap().end
    }

    fn is_state_limit_error(response: &StreamResponse) -> bool {
        matches!(
            response,
            StreamResponse::ErrorResponse { error_message, provider, .. }
                if provider == "openai" && error_message.contains("state limit exceeded")
        )
    }
}

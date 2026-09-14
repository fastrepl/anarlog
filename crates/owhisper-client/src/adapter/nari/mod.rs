use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use anlg_ws_client::client::Message;
use base64::{Engine, engine::general_purpose::STANDARD};
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::Deserialize;

use super::{LanguageQuality, LanguageSupport, RealtimeSttAdapter, parsing::WordBuilder};
use crate::providers::Provider;

pub(crate) const DEFAULT_MODEL: &str = "qwen3-asr-fast:free";
const MODELS: &[&str] = &[
    "qwen3-asr-fast:free",
    "qwen3-asr:free",
    "qwen3-asr-fast",
    "qwen3-asr",
];
const LANGUAGES: &[&str] = &[
    "ar", "cs", "da", "de", "el", "en", "es", "fa", "fi", "fil", "fr", "hi", "hu", "id", "it",
    "ja", "ko", "mk", "ms", "nl", "pl", "pt", "ro", "ru", "sv", "th", "tr", "vi", "yue", "zh",
];
const FINALIZE_ID: &str = "anarlog_end_of_input";

#[derive(Clone, Default)]
pub struct NariAdapter {
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    samples_sent: u64,
    cursor: f64,
    order: VecDeque<String>,
    items: HashMap<String, Item>,
    completed: VecDeque<String>,
    end_acknowledged: bool,
    finalized: bool,
}

#[derive(Default)]
struct Item {
    start: Option<f64>,
    end: Option<f64>,
    completed: Option<Event>,
}

#[derive(Default, Deserialize)]
struct Event {
    #[serde(rename = "type")]
    kind: String,
    item_id: Option<String>,
    transcript: Option<String>,
    language: Option<String>,
    audio_start_ms: Option<u64>,
    audio_end_ms: Option<u64>,
    client_event_id: Option<String>,
    usage: Option<Usage>,
    error: Option<ProviderFailure>,
}

#[derive(Deserialize)]
struct Usage {
    input_audio_seconds: f64,
}

#[derive(Deserialize)]
struct ProviderFailure {
    code: String,
    message: String,
}

impl NariAdapter {
    pub fn language_support_live(
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        let known_model = model
            .is_none_or(|model| crate::providers::is_meta_model(model) || MODELS.contains(&model));
        if known_model
            && languages.iter().all(|language| {
                let code = language.iso639().code();
                LANGUAGES.contains(&code) || code == "tl"
            })
        {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        } else {
            LanguageSupport::NotSupported
        }
    }
}

impl RealtimeSttAdapter for NariAdapter {
    fn fork_session(&self) -> Self {
        Self::default()
    }

    fn provider_name(&self) -> &'static str {
        "nari"
    }

    fn initial_response_type(&self) -> Option<&'static str> {
        Some("session.configured")
    }

    fn required_sample_rate(&self) -> Option<u32> {
        Some(16_000)
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> bool {
        Self::language_support_live(languages, model).is_supported()
    }

    fn supports_native_multichannel(&self) -> bool {
        false
    }

    fn build_ws_url(&self, api_base: &str, _params: &ListenParams, _channels: u8) -> url::Url {
        let parsed = url::Url::parse(api_base)
            .unwrap_or_else(|_| Provider::Nari.default_api_base().parse().unwrap());
        let mut url = super::build_url_with_scheme(
            &parsed,
            Provider::Nari.default_ws_host(),
            Provider::Nari.ws_path(),
            true,
        );
        url.query_pairs_mut().append_pair("intent", "transcription");
        url
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|key| Provider::Nari.build_auth_header(key))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn initial_message(
        &self,
        _api_key: Option<&str>,
        params: &ListenParams,
        _channels: u8,
    ) -> Option<Message> {
        let model = params
            .model
            .as_deref()
            .filter(|model| !crate::providers::is_meta_model(model))
            .unwrap_or(DEFAULT_MODEL);
        let language = if params.languages.len() == 1 {
            let code = params.languages[0].iso639().code();
            Some(if code == "tl" { "fil" } else { code })
        } else {
            None
        };
        Some(Message::Text(serde_json::json!({
            "type": "session.configure",
            "session": {"model": model, "language": language, "prompt": params.keywords.join(", "),
                "turn_detection": {"type": "server_vad"}}
        }).to_string().into()))
    }

    fn audio_to_message(&self, audio: bytes::Bytes) -> Message {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .samples_sent += (audio.len() / 2) as u64;
        Message::Text(serde_json::json!({"type": "input_audio_buffer.append", "audio": STANDARD.encode(audio)}).to_string().into())
    }

    fn finalize_message(&self) -> Message {
        Message::Text(
            serde_json::json!({"type": "input_audio_buffer.commit", "event_id": FINALIZE_ID})
                .to_string()
                .into(),
        )
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let Ok(event) = serde_json::from_str::<Event>(raw) else {
            return vec![];
        };
        if event.kind == "error" {
            return event
                .error
                .map(|error| {
                    vec![StreamResponse::ErrorResponse {
                        error_code: match error.code.as_str() {
                            "INVALID_API_KEY" => Some(401),
                            "INSUFFICIENT_CREDITS" => Some(402),
                            "UPSTREAM_RATE_LIMITED"
                            | "FREE_DAILY_LIMIT_EXCEEDED"
                            | "CONCURRENCY_LIMIT_EXCEEDED" => Some(429),
                            _ => None,
                        },
                        error_message: format!("{}: {}", error.code, error.message),
                        provider: "nari".into(),
                    }]
                })
                .unwrap_or_default();
        }
        if !matches!(
            event.kind.as_str(),
            "input_audio_buffer.speech_started"
                | "input_audio_buffer.speech_stopped"
                | "input_audio_buffer.committed"
                | "input_audio_buffer.commit_empty"
                | "transcript.partial"
                | "transcript.completed"
        ) {
            return vec![];
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(
            event.kind.as_str(),
            "input_audio_buffer.committed" | "input_audio_buffer.commit_empty"
        ) && event.client_event_id.as_deref() == Some(FINALIZE_ID)
        {
            state.end_acknowledged = true;
        }
        let mut responses = vec![];
        if let Some(id) = event.item_id.clone() {
            if state.completed.contains(&id) {
                return vec![];
            }
            if !state.items.contains_key(&id) {
                if state.items.len() >= 64 || id.len() > 1024 {
                    return vec![StreamResponse::ErrorResponse {
                        error_code: None,
                        error_message: "Too many pending Nari utterances or invalid item ID".into(),
                        provider: "nari".into(),
                    }];
                }
                state.order.push_back(id.clone());
                state.items.insert(id.clone(), Item::default());
            }
            let cursor = state.cursor;
            let sent_end = state.samples_sent as f64 / 16_000.0;
            let item = state.items.get_mut(&id).unwrap();
            match event.kind.as_str() {
                "input_audio_buffer.speech_started" => {
                    item.start = event.audio_start_ms.map(|ms| ms as f64 / 1000.0)
                }
                "input_audio_buffer.speech_stopped" => {
                    item.end = event.audio_end_ms.map(|ms| ms as f64 / 1000.0)
                }
                "transcript.partial" => {
                    let start = item.start.unwrap_or(cursor);
                    responses.push(transcript_response(
                        event.transcript.as_deref().unwrap_or(""),
                        start,
                        item.end.unwrap_or(sent_end).max(start),
                        false,
                        event.language.clone(),
                    ));
                }
                "transcript.completed" => item.completed = Some(event),
                _ => {}
            }
        }
        while let Some(id) = state.order.front().cloned() {
            if state
                .items
                .get(&id)
                .and_then(|item| item.completed.as_ref())
                .is_none()
            {
                break;
            }
            let item = state.items.remove(&id).unwrap();
            state.order.pop_front();
            let event = item.completed.unwrap();
            let start = item.start.unwrap_or(state.cursor);
            let duration = event
                .usage
                .map(|usage| usage.input_audio_seconds)
                .filter(|duration| duration.is_finite() && *duration >= 0.0)
                .unwrap_or(0.0);
            let end = item.end.unwrap_or(start + duration).max(start);
            state.cursor = state.cursor.max(end);
            responses.push(transcript_response(
                event.transcript.as_deref().unwrap_or(""),
                start,
                end,
                true,
                event.language,
            ));
            state.completed.push_back(id);
            if state.completed.len() > 64 {
                state.completed.pop_front();
            }
        }
        if state.end_acknowledged && state.items.is_empty() && !state.finalized {
            state.finalized = true;
            let mut marker = transcript_response("", state.cursor, state.cursor, true, None);
            if let StreamResponse::TranscriptResponse { from_finalize, .. } = &mut marker {
                *from_finalize = true;
            }
            responses.push(marker);
        }
        responses
    }
}

fn transcript_response(
    text: &str,
    start: f64,
    end: f64,
    is_final: bool,
    language: Option<String>,
) -> StreamResponse {
    // Nari exposes utterance timing, not word alignment. Preserve one timed
    // segment so the transcript store retains the text without invented word times.
    let words = if text.is_empty() {
        vec![]
    } else {
        vec![
            WordBuilder::new(text)
                .start(start)
                .end(end)
                .language(language.clone())
                .build(),
        ]
    };
    StreamResponse::TranscriptResponse {
        is_final,
        speech_final: is_final,
        from_finalize: false,
        start,
        duration: end - start,
        channel: Channel {
            alternatives: vec![Alternatives {
                transcript: text.into(),
                words,
                confidence: 1.0,
                languages: language.into_iter().collect(),
            }],
        },
        metadata: Metadata::default(),
        channel_index: vec![0, 1],
    }
}

#[cfg(test)]
mod tests;

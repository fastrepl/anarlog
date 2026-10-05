use super::{
    LanguageQuality, LanguageSupport, RealtimeSttAdapter,
    parsing::WordBuilder,
    segment::{self, SegmentState},
};
use anlg_ws_client::client::Message;
use base64::{Engine, engine::general_purpose::STANDARD};
use owhisper_interface::{ListenParams, stream::StreamResponse};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct NvidiaAdapter {
    state: Arc<Mutex<SegmentState>>,
    token: Arc<Mutex<Option<String>>>,
}
impl NvidiaAdapter {
    pub fn language_support_live(
        languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> LanguageSupport {
        // The selected NIM deployment determines language coverage.
        if languages.len() <= 1 {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        } else {
            LanguageSupport::NotSupported
        }
    }
}
impl RealtimeSttAdapter for NvidiaAdapter {
    fn fork_session(&self) -> Self {
        Self::default()
    }
    fn provider_name(&self) -> &'static str {
        "nvidia"
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
    fn build_ws_url(&self, base: &str, _: &ListenParams, _: u8) -> url::Url {
        let mut url = segment::ws_url(base, "http://localhost:9000", "/v1/realtime");
        url.query_pairs_mut().append_pair("intent", "transcription");
        url
    }
    async fn build_ws_url_with_api_key(
        &self,
        base: &str,
        params: &ListenParams,
        channels: u8,
        key: Option<&str>,
    ) -> Option<url::Url> {
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let url = self.build_ws_url(base, params, channels);
        let mut endpoint = url.clone();
        endpoint
            .set_scheme(if url.scheme() == "ws" {
                "http"
            } else {
                "https"
            })
            .ok()?;
        endpoint.set_path("/v1/realtime/transcription_sessions");
        endpoint.set_query(None);
        let mut request = crate::http_client::create_client()
            .post(endpoint)
            .timeout(std::time::Duration::from_secs(10));
        if let Some(key) = key.filter(|key| !key.is_empty()) {
            request = request.bearer_auth(key);
        }
        let response = request.send().await.ok()?.error_for_status().ok()?;
        let payload: Value = response.json().await.ok()?;
        let token = payload["client_secret"]["value"]
            .as_str()
            .map(str::to_owned);
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) = token;
        Some(url)
    }
    fn build_auth_header(&self, key: Option<&str>) -> Option<(&'static str, String)> {
        if let Some(token) = self
            .token
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_deref()
        {
            Some((
                "Sec-WebSocket-Protocol",
                format!("realtime, realtime-token.{token}"),
            ))
        } else {
            key.filter(|key| !key.is_empty())
                .map(|key| ("Authorization", format!("Bearer {key}")))
        }
    }
    fn keep_alive_message(&self) -> Option<Message> {
        None
    }
    fn finalize_message(&self) -> Message {
        segment::message(json!({"type":"input_audio_buffer.done"}))
    }
    fn initial_message(&self, _: Option<&str>, params: &ListenParams, _: u8) -> Option<Message> {
        Some(segment::message(
            json!({"type":"transcription_session.update","session":{
                "modalities":["text"],"input_audio_format":"pcm16",
                "input_audio_transcription":{"model":params.model.as_deref().unwrap_or("nemotron-asr-streaming"),"language":params.languages.first().map(|lang| if lang.iso639().code() == "en" && lang.region().is_none() { "en-US".into() } else { lang.bcp47_code() }).unwrap_or_else(|| "en-US".into())},
                "input_audio_params":{"sample_rate_hz":16000,"num_channels":1},
                "recognition_config":{"enable_automatic_punctuation":true,"enable_word_time_offsets":true},
                "speaker_diarization":{"enable_speaker_diarization":params.num_speakers != Some(1)},
                "word_boosting":{"enable_word_boosting":!params.keywords.is_empty(),"word_boosting_list":params.keywords}
            }}),
        ))
    }
    fn audio_to_message(&self, audio: bytes::Bytes) -> Message {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .audio(audio.len(), 16000);
        segment::message(json!({"type":"input_audio_buffer.append","audio":STANDARD.encode(audio)}))
    }
    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let Ok(value) = serde_json::from_str::<Value>(raw) else {
            return vec![];
        };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match value["type"].as_str().unwrap_or("") {
            "error" | "conversation.item.input_audio_transcription.failed" => {
                vec![segment::error("nvidia", &value["error"])]
            }
            "conversation.item.input_audio_transcription.delta" => {
                let Some(text) = value["delta"].as_str() else {
                    return vec![];
                };
                if state.text.len() + text.len() > 1024 * 1024 {
                    return vec![segment::error(
                        "nvidia",
                        &json!({"message":"Unterminated transcript segment exceeds the size limit"}),
                    )];
                }
                state.text.push_str(text);
                let text = state.text.clone();
                let end = state.end(16000);
                vec![state.response(&text, end, false)]
            }
            "conversation.item.input_audio_transcription.completed" => {
                let Some(text) = value["transcript"].as_str() else {
                    return vec![];
                };
                let end = state.end(16000);
                let mut response = state.response(text, end, true);
                if let Some(words) = value["words_info"]["words"].as_array() {
                    let words: Vec<_> = words
                        .iter()
                        .filter_map(|word| {
                            Some(
                                WordBuilder::new(word["word"].as_str()?)
                                    .start(word["start_time"].as_f64()?)
                                    .end(word["end_time"].as_f64()?)
                                    .confidence(word["confidence"].as_f64().unwrap_or(1.0))
                                    .speaker(
                                        word["speaker_tag"]
                                            .as_i64()
                                            .and_then(|tag| i32::try_from(tag).ok()),
                                    )
                                    .build(),
                            )
                        })
                        .collect();
                    if let StreamResponse::TranscriptResponse {
                        start,
                        duration,
                        channel,
                        ..
                    } = &mut response
                        && let (Some(first), Some(last)) = (words.first(), words.last())
                    {
                        *start = first.start;
                        *duration = last.end - first.start;
                        state.cursor = last.end;
                        channel.alternatives[0].words = words;
                    }
                }
                let mut responses = vec![response];
                if value["is_last_result"].as_bool() == Some(true) {
                    responses.push(segment::finished(end));
                }
                responses
            }
            _ => vec![],
        }
    }
}

use super::{
    LanguageQuality, LanguageSupport, RealtimeSttAdapter,
    segment::{self, SegmentState},
};
use anlg_ws_client::client::Message;
use base64::{Engine, engine::general_purpose::STANDARD};
use owhisper_interface::{ListenParams, stream::StreamResponse};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct GradiumAdapter {
    state: Arc<Mutex<SegmentState>>,
}
impl GradiumAdapter {
    pub fn language_support_live(
        languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> LanguageSupport {
        if languages
            .iter()
            .all(|lang| ["en", "fr", "es", "pt", "de"].contains(&lang.iso639().code()))
        {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        } else {
            LanguageSupport::NotSupported
        }
    }
}
impl RealtimeSttAdapter for GradiumAdapter {
    fn fork_session(&self) -> Self {
        Self::default()
    }
    fn provider_name(&self) -> &'static str {
        "gradium"
    }
    fn initial_response_type(&self) -> Option<&'static str> {
        Some("ready")
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
        segment::ws_url(base, "https://api.gradium.ai", "/api/speech/asr")
    }
    fn build_auth_header(&self, key: Option<&str>) -> Option<(&'static str, String)> {
        key.map(|key| ("x-api-key", key.into()))
    }
    fn keep_alive_message(&self) -> Option<Message> {
        None
    }
    fn finalize_message(&self) -> Message {
        segment::message(json!({"type":"end_of_stream"}))
    }
    fn initial_message(&self, _: Option<&str>, params: &ListenParams, _: u8) -> Option<Message> {
        let language = if params.languages.len() == 1 {
            params.languages[0].iso639().code()
        } else {
            "any"
        };
        let mut config = json!({"language":language});
        if !params.keywords.is_empty() {
            config["keywords"] = json!({"words":params.keywords,"boost":3});
        }
        Some(segment::message(
            json!({"type":"setup","model_name":params.model.as_deref().unwrap_or("default"),"input_format":"pcm_16000","json_config":config}),
        ))
    }
    fn audio_to_message(&self, audio: bytes::Bytes) -> Message {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .audio(audio.len(), 16000);
        segment::message(json!({"type":"audio","audio":STANDARD.encode(audio)}))
    }
    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let Ok(value) = serde_json::from_str::<Value>(raw) else {
            return vec![];
        };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match value["type"].as_str().unwrap_or("") {
            "error" => vec![segment::error("gradium", &value)],
            "text" => {
                let Some(text) = value["text"].as_str() else {
                    return vec![];
                };
                if state.text.is_empty() {
                    state.start = value["start_s"].as_f64();
                }
                if !state.text.is_empty() && !text.starts_with(char::is_whitespace) {
                    state.text.push(' ');
                }
                if state.text.len() + text.len() > 1024 * 1024 {
                    return vec![segment::error(
                        "gradium",
                        &json!({"message":"Unterminated transcript segment exceeds the size limit"}),
                    )];
                }
                state.text.push_str(text);
                let text = state.text.clone();
                let end = state.end(16000);
                vec![state.response(&text, end, false)]
            }
            "end_text" => {
                let text = state.text.clone();
                let end = value["stop_s"].as_f64().unwrap_or_else(|| state.end(16000));
                vec![state.response(&text, end, true)]
            }
            "end_of_stream" => vec![segment::finished(state.end(16000))],
            _ => vec![],
        }
    }
}

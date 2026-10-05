use super::{
    LanguageQuality, LanguageSupport, RealtimeSttAdapter,
    segment::{self, SegmentState},
};
use anlg_ws_client::client::Message;
use owhisper_interface::{ListenParams, stream::StreamResponse};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct AlebexAdapter {
    state: Arc<Mutex<SegmentState>>,
}
impl AlebexAdapter {
    pub fn language_support_live(
        languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> LanguageSupport {
        // Alebex's public model card currently benchmarks English only.
        if languages.iter().all(|lang| lang.iso639().code() == "en") {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        } else {
            LanguageSupport::NotSupported
        }
    }
}
impl RealtimeSttAdapter for AlebexAdapter {
    fn fork_session(&self) -> Self {
        Self::default()
    }
    fn provider_name(&self) -> &'static str {
        "alebex"
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
        segment::ws_url(base, "https://asr.alebex.ai", "/asr/ws")
    }
    fn build_auth_header(&self, key: Option<&str>) -> Option<(&'static str, String)> {
        key.map(|key| ("Sec-WebSocket-Protocol", format!("alebex.token.{key}")))
    }
    fn keep_alive_message(&self) -> Option<Message> {
        None
    }
    fn finalize_message(&self) -> Message {
        segment::message(json!({"type":"end"}))
    }
    fn initial_message(&self, _: Option<&str>, params: &ListenParams, _: u8) -> Option<Message> {
        Some(segment::message(
            json!({"type":"start","language":params.languages.first().map(|lang| lang.iso639().code())}),
        ))
    }
    fn audio_to_message(&self, audio: bytes::Bytes) -> Message {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .audio(audio.len(), 16000);
        Message::Binary(audio)
    }
    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let Ok(value) = serde_json::from_str::<Value>(raw) else {
            return vec![];
        };
        let is_final = match value["type"].as_str().unwrap_or("") {
            "partial" => false,
            "final" => true,
            "error" => {
                return vec![segment::error(
                    "alebex",
                    value.get("error").unwrap_or(&value),
                )];
            }
            _ => return vec![],
        };
        let Some(text) = value["text"].as_str() else {
            return vec![];
        };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let end = state.end(16000);
        vec![state.response(text, end, is_final)]
    }
}

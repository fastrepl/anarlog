use super::{
    LanguageQuality, LanguageSupport, RealtimeSttAdapter,
    segment::{self, SegmentState},
};
use anlg_ws_client::client::Message;
use owhisper_interface::{ListenParams, stream::StreamResponse};
use serde_json::Value;
use std::sync::{Arc, Mutex};

const ENGLISH: &str = "velma-2-stt-streaming-english-v2";
const MULTILINGUAL: &str = "velma-2-stt-streaming-multilingual-vfast";
#[derive(Clone, Default)]
pub struct ModulateAdapter {
    state: Arc<Mutex<SegmentState>>,
}
impl ModulateAdapter {
    pub fn language_support_live(
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        let codes: &[&str] = if model == Some(ENGLISH) {
            &["en"]
        } else {
            &[
                "bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "hu", "it", "lv",
                "lt", "mt", "pl", "pt", "ro", "ru", "sk", "sl", "es", "sv", "uk",
            ]
        };
        if languages
            .iter()
            .all(|lang| codes.contains(&lang.iso639().code()))
        {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        } else {
            LanguageSupport::NotSupported
        }
    }
}
impl RealtimeSttAdapter for ModulateAdapter {
    fn fork_session(&self) -> Self {
        Self::default()
    }
    fn provider_name(&self) -> &'static str {
        "modulate"
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
    fn build_ws_url(&self, base: &str, params: &ListenParams, _: u8) -> url::Url {
        let model = if params.model.as_deref() == Some(ENGLISH) {
            ENGLISH
        } else {
            MULTILINGUAL
        };
        let mut url = segment::ws_url(
            base,
            "https://platform.modulate.ai",
            &format!("/api/{model}"),
        );
        url.query_pairs_mut()
            .append_pair("audio_format", "s16le")
            .append_pair("sample_rate", "16000")
            .append_pair("num_channels", "1")
            .append_pair("endpointing", "true")
            .append_pair(
                "diarize",
                if params.num_speakers == Some(1) {
                    "false"
                } else {
                    "true"
                },
            );
        url
    }
    fn build_ws_url_with_api_key(
        &self,
        base: &str,
        params: &ListenParams,
        channels: u8,
        key: Option<&str>,
    ) -> impl std::future::Future<Output = Option<url::Url>> + Send {
        let mut url = self.build_ws_url(base, params, channels);
        let key = key.filter(|key| !key.is_empty()).map(str::to_owned);
        async move {
            url.query_pairs_mut().append_pair("api_key", &key?);
            Some(url)
        }
    }
    fn build_auth_header(&self, _: Option<&str>) -> Option<(&'static str, String)> {
        None
    }
    fn keep_alive_message(&self) -> Option<Message> {
        None
    }
    fn finalize_message(&self) -> Message {
        Message::Text("".into())
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
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match value["type"].as_str().unwrap_or("") {
            "error" => vec![segment::error("modulate", &value["error"])],
            "partial_utterance" => {
                let Some(text) = value["partial_utterance"]["text"].as_str() else {
                    return vec![];
                };
                let end = state.end(16000);
                vec![state.response(text, end, false)]
            }
            "utterance" => {
                let utterance = &value["utterance"];
                let Some(text) = utterance["text"].as_str() else {
                    return vec![];
                };
                let start = utterance["start_ms"]
                    .as_f64()
                    .map(|ms| ms / 1000.0)
                    .unwrap_or(state.cursor);
                let end = utterance["duration_ms"]
                    .as_f64()
                    .map(|ms| start + ms / 1000.0)
                    .unwrap_or_else(|| state.end(16000))
                    .max(start);
                let speaker = utterance["speaker"]
                    .as_i64()
                    .and_then(|speaker| i32::try_from(speaker).ok());
                state.cursor = end;
                vec![segment::response(text, start, end, true, speaker)]
            }
            "done" => vec![segment::finished(state.end(16000))],
            _ => vec![],
        }
    }
}

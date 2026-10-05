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
pub struct InworldAdapter {
    state: Arc<Mutex<SegmentState>>,
}

impl InworldAdapter {
    pub fn language_support_live(
        languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> LanguageSupport {
        let supported = languages.iter().all(|lang| {
            [
                "ar", "yue", "zh", "cs", "da", "nl", "en", "fil", "tl", "fi", "fr", "de", "el",
                "hi", "hu", "id", "it", "ja", "ko", "mk", "ms", "fa", "pl", "pt", "ro", "ru", "es",
                "sv", "th", "tr", "vi",
            ]
            .contains(&lang.iso639().code())
        });
        if supported {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        } else {
            LanguageSupport::NotSupported
        }
    }
}

impl RealtimeSttAdapter for InworldAdapter {
    fn fork_session(&self) -> Self {
        Self::default()
    }
    fn provider_name(&self) -> &'static str {
        "inworld"
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
        segment::ws_url(
            base,
            "https://api.inworld.ai",
            "/stt/v1/transcribe:streamBidirectional",
        )
    }
    fn build_auth_header(&self, key: Option<&str>) -> Option<(&'static str, String)> {
        key.map(|key| ("Authorization", format!("Basic {key}")))
    }
    fn keep_alive_message(&self) -> Option<Message> {
        None
    }
    fn finalize_message(&self) -> Message {
        segment::message(json!({"closeStream":{}}))
    }
    fn initial_message(&self, _: Option<&str>, params: &ListenParams, _: u8) -> Option<Message> {
        let mut config = json!({"modelId":params.model.as_deref().unwrap_or("inworld/inworld-stt-1"),"audioEncoding":"LINEAR16","sampleRateHertz":16000,"numberOfChannels":1,"includeWordTimestamps":true,"enableSpeakerDiarization":params.num_speakers != Some(1)});
        if params.languages.len() == 1 {
            let lang = &params.languages[0];
            config["language"] = json!(lang.iso639().code());
        }
        Some(segment::message(json!({"transcribeConfig":config})))
    }
    fn audio_to_message(&self, audio: bytes::Bytes) -> Message {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .audio(audio.len(), 16000);
        segment::message(json!({"audioChunk":{"content":STANDARD.encode(audio)}}))
    }
    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let Ok(value) = serde_json::from_str::<Value>(raw) else {
            return vec![];
        };
        if let Some(error) = value.get("error") {
            return vec![segment::error("inworld", error)];
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let result = &value["result"];
        if let Some(ms) = result["speechStarted"]["startTimeMs"].as_f64() {
            state.start = Some(ms / 1000.0);
        }
        let transcript = &result["transcription"];
        let Some(text) = transcript["transcript"].as_str() else {
            return vec![];
        };
        let end = state.end(16000);
        let mut response =
            state.response(text, end, transcript["isFinal"].as_bool().unwrap_or(false));
        if let Some(words) = transcript["wordTimestamps"].as_array() {
            let words: Vec<_> = words
                .iter()
                .filter_map(|word| {
                    Some(
                        super::parsing::WordBuilder::new(word["word"].as_str()?)
                            .start(word["startTimeMs"].as_f64()? / 1000.0)
                            .end(word["endTimeMs"].as_f64()? / 1000.0)
                            .confidence(word["confidence"].as_f64().unwrap_or(1.0))
                            .speaker(
                                word["speaker"]
                                    .as_i64()
                                    .and_then(|speaker| i32::try_from(speaker).ok()),
                            )
                            .build(),
                    )
                })
                .collect();
            if let StreamResponse::TranscriptResponse {
                start,
                duration,
                channel,
                is_final,
                ..
            } = &mut response
                && let (Some(first), Some(last)) = (words.first(), words.last())
            {
                *start = first.start;
                *duration = last.end - first.start;
                if *is_final {
                    state.cursor = last.end;
                }
                channel.alternatives[0].words = words;
            }
        }
        vec![response]
    }
}

use anlg_ws_client::client::Message;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde_json::Value;

use super::parsing::WordBuilder;

#[derive(Default)]
pub(super) struct SegmentState {
    pub samples: u64,
    pub cursor: f64,
    pub text: String,
    pub start: Option<f64>,
}

impl SegmentState {
    pub fn audio(&mut self, bytes: usize, rate: u32) -> f64 {
        self.samples += bytes as u64 / 2;
        self.samples as f64 / rate as f64
    }

    pub fn end(&self, rate: u32) -> f64 {
        self.samples as f64 / rate as f64
    }

    pub fn response(&mut self, text: &str, end: f64, is_final: bool) -> StreamResponse {
        let start = self.start.unwrap_or(self.cursor);
        let response = response(text, start, end.max(start), is_final, None);
        if is_final {
            self.cursor = end.max(start);
            self.start = None;
            self.text.clear();
        }
        response
    }
}

pub(super) fn response(
    text: &str,
    start: f64,
    end: f64,
    is_final: bool,
    speaker: Option<i32>,
) -> StreamResponse {
    // These protocols expose segment timing, so preserve one timed segment
    // instead of fabricating word alignment.
    let words = if text.is_empty() {
        vec![]
    } else {
        vec![
            WordBuilder::new(text)
                .start(start)
                .end(end)
                .speaker(speaker)
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
                languages: vec![],
            }],
        },
        metadata: Metadata::default(),
        channel_index: vec![0, 1],
    }
}

pub(super) fn finished(end: f64) -> StreamResponse {
    let mut response = response("", end, end, true, None);
    if let StreamResponse::TranscriptResponse { from_finalize, .. } = &mut response {
        *from_finalize = true;
    }
    response
}

pub(super) fn error(provider: &str, value: &Value) -> StreamResponse {
    let message = value
        .as_str()
        .or_else(|| value.get("message").and_then(Value::as_str))
        .unwrap_or("Transcription provider returned an error");
    StreamResponse::ErrorResponse {
        error_code: value
            .get("code")
            .and_then(Value::as_u64)
            .and_then(|code| i32::try_from(code).ok()),
        error_message: message.into(),
        provider: provider.into(),
    }
}

pub(super) fn message(value: Value) -> Message {
    Message::Text(value.to_string().into())
}

pub(super) fn ws_url(base: &str, default: &str, path: &str) -> url::Url {
    let mut url = url::Url::parse(if base.is_empty() { default } else { base })
        .expect("invalid STT base URL");
    let scheme = if matches!(url.scheme(), "http" | "ws") {
        "ws"
    } else {
        "wss"
    };
    url.set_scheme(scheme).expect("invalid STT URL scheme");
    let query: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| key != "provider")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.set_query(None);
    url.query_pairs_mut().extend_pairs(query);
    url.set_path(path);
    url
}

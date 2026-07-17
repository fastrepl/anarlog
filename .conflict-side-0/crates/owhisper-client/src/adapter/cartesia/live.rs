use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::Deserialize;

use crate::adapter::RealtimeSttAdapter;

use super::{API_VERSION, CartesiaAdapter};

const LIVE_MODEL: &str = "ink-2";

impl RealtimeSttAdapter for CartesiaAdapter {
    fn provider_name(&self) -> &'static str {
        "cartesia"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        CartesiaAdapter::is_supported_languages_live(languages)
    }

    fn supports_native_multichannel(&self) -> bool {
        false
    }

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, _channels: u8) -> url::Url {
        let (mut url, existing_params) = CartesiaAdapter::build_ws_url_from_base(api_base);
        let model = resolve_live_model(params.model.as_deref());

        {
            let mut query = url.query_pairs_mut();
            for (key, value) in &existing_params {
                query.append_pair(key, value);
            }
            query.append_pair("model", model);
            query.append_pair("encoding", "pcm_s16le");
            query.append_pair("sample_rate", &params.sample_rate.to_string());
            query.append_pair("cartesia_version", API_VERSION);
        }

        url
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.map(|key| ("X-API-Key", key.to_string()))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn finalize_message(&self) -> Message {
        Message::Text(r#"{"type":"close"}"#.into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let event: CartesiaEvent = match serde_json::from_str(raw) {
            Ok(event) => event,
            Err(error) => {
                tracing::warn!(
                    error = ?error,
                    hyprnote.payload.size_bytes = raw.len() as u64,
                    "cartesia_json_parse_failed"
                );
                return vec![];
            }
        };

        match event {
            CartesiaEvent::Connected { request_id } => {
                tracing::debug!(request.id = %request_id, "cartesia_connected");
                vec![]
            }
            CartesiaEvent::TurnStart { .. } => vec![StreamResponse::SpeechStartedResponse {
                channel: vec![0],
                timestamp: 0.0,
            }],
            CartesiaEvent::TurnUpdate {
                transcript,
                request_id,
            }
            | CartesiaEvent::TurnEagerEnd {
                transcript,
                request_id,
            } => build_transcript_response(transcript, request_id, false),
            CartesiaEvent::TurnEnd {
                transcript,
                request_id,
            } => build_transcript_response(transcript, request_id, true),
            CartesiaEvent::TurnResume { .. } => vec![],
            CartesiaEvent::Done { request_id } => vec![StreamResponse::TerminalResponse {
                request_id,
                created: chrono_like_now(),
                duration: 0.0,
                channels: 1,
            }],
            CartesiaEvent::Error {
                message,
                title,
                status_code,
                ..
            } => vec![StreamResponse::ErrorResponse {
                error_code: status_code,
                error_message: message.unwrap_or(title),
                provider: "cartesia".to_string(),
            }],
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum CartesiaEvent {
    #[serde(rename = "connected")]
    Connected { request_id: String },
    #[serde(rename = "turn.start")]
    TurnStart {
        #[serde(rename = "request_id")]
        _request_id: String,
    },
    #[serde(rename = "turn.update")]
    TurnUpdate {
        transcript: String,
        request_id: String,
    },
    #[serde(rename = "turn.eager_end")]
    TurnEagerEnd {
        transcript: String,
        request_id: String,
    },
    #[serde(rename = "turn.resume")]
    TurnResume {
        #[serde(rename = "request_id")]
        _request_id: String,
    },
    #[serde(rename = "turn.end")]
    TurnEnd {
        transcript: String,
        request_id: String,
    },
    #[serde(rename = "done")]
    Done { request_id: String },
    #[serde(rename = "error")]
    Error {
        title: String,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        status_code: Option<i32>,
    },
}

fn resolve_live_model(model: Option<&str>) -> &str {
    match model {
        Some(model) if crate::providers::is_meta_model(model) => LIVE_MODEL,
        Some("ink-whisper") => LIVE_MODEL,
        Some(model) => model,
        None => LIVE_MODEL,
    }
}

fn build_transcript_response(
    transcript: String,
    request_id: String,
    is_final: bool,
) -> Vec<StreamResponse> {
    vec![StreamResponse::TranscriptResponse {
        start: 0.0,
        duration: 0.0,
        is_final,
        speech_final: is_final,
        from_finalize: false,
        channel: Channel {
            alternatives: vec![Alternatives {
                transcript,
                words: Vec::new(),
                confidence: 1.0,
                languages: vec!["en".to_string()],
            }],
        },
        metadata: Metadata {
            request_id,
            model_info: owhisper_interface::stream::ModelInfo {
                name: LIVE_MODEL.to_string(),
                version: String::new(),
                arch: String::new(),
            },
            model_uuid: String::new(),
            extra: None,
        },
        channel_index: vec![0, 1],
    }]
}

fn chrono_like_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_ws_url_uses_ink_2_turn_endpoint() {
        let params = ListenParams {
            model: Some("ink-2".to_string()),
            sample_rate: 16_000,
            ..Default::default()
        };
        let adapter = CartesiaAdapter;

        let url = adapter.build_ws_url("https://api.cartesia.ai", &params, 1);

        assert_eq!(url.scheme(), "wss");
        assert_eq!(url.host_str(), Some("api.cartesia.ai"));
        assert_eq!(url.path(), "/stt/turns/websocket");
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query.get("model").map(String::as_str), Some("ink-2"));
        assert_eq!(query.get("encoding").map(String::as_str), Some("pcm_s16le"));
        assert_eq!(query.get("sample_rate").map(String::as_str), Some("16000"));
        assert_eq!(
            query.get("cartesia_version").map(String::as_str),
            Some(API_VERSION)
        );
    }

    #[test]
    fn turn_end_becomes_final_transcript() {
        let adapter = CartesiaAdapter;
        let responses = adapter
            .parse_response(r#"{"type":"turn.end","transcript":"hello","request_id":"req_123"}"#);

        assert_eq!(responses.len(), 1);
        let StreamResponse::TranscriptResponse {
            is_final,
            speech_final,
            channel,
            metadata,
            ..
        } = &responses[0]
        else {
            panic!("expected transcript response");
        };

        assert!(*is_final);
        assert!(*speech_final);
        assert_eq!(metadata.request_id, "req_123");
        assert_eq!(channel.alternatives[0].transcript, "hello");
    }

    #[test]
    fn turn_update_becomes_partial_transcript() {
        let adapter = CartesiaAdapter;
        let responses = adapter.parse_response(
            r#"{"type":"turn.update","transcript":"hello","request_id":"req_123"}"#,
        );

        let StreamResponse::TranscriptResponse {
            is_final,
            speech_final,
            ..
        } = &responses[0]
        else {
            panic!("expected transcript response");
        };

        assert!(!*is_final);
        assert!(!*speech_final);
    }
}

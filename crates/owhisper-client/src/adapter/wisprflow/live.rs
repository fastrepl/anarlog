use anlg_ws_client::client::Message;
use base64::{Engine, engine::general_purpose::STANDARD};
use owhisper_interface::{
    ListenParams,
    stream::{Alternatives, Channel, Metadata, StreamResponse},
};
use serde_json::json;

use super::WisprFlowAdapter;
use crate::adapter::RealtimeSttAdapter;

#[derive(Default)]
pub(super) struct Session {
    packets: usize,
    packet_bytes: usize,
    samples: usize,
}

impl RealtimeSttAdapter for WisprFlowAdapter {
    fn fork_session(&self) -> Self {
        Self::default()
    }
    fn provider_name(&self) -> &'static str {
        "wisprflow"
    }
    fn initial_response_type(&self) -> Option<&'static str> {
        Some("auth")
    }
    fn initial_response_field(&self) -> &'static str {
        "status"
    }
    fn required_sample_rate(&self) -> Option<u32> {
        Some(16_000)
    }
    fn is_supported_languages(&self, _: &[anlg_language::Language], _: Option<&str>) -> bool {
        true
    }
    fn supports_native_multichannel(&self) -> bool {
        false
    }

    fn build_ws_url(&self, base: &str, _: &ListenParams, _: u8) -> url::Url {
        let mut url = Self::endpoint(base, "/api/v1/dash/ws")
            .unwrap_or_else(|_| Self::endpoint("", "/api/v1/dash/ws").unwrap());
        crate::adapter::set_scheme_from_host(&mut url);
        url
    }

    async fn build_ws_url_with_api_key(
        &self,
        base: &str,
        _params: &ListenParams,
        _channels: u8,
        api_key: Option<&str>,
    ) -> Option<url::Url> {
        let mut url = Self::endpoint(base, "/api/v1/dash/ws").ok()?;
        crate::adapter::set_scheme_from_host(&mut url);
        url.query_pairs_mut()
            .append_pair("api_key", &format!("Bearer {}", api_key?));
        Some(url)
    }

    fn build_auth_header(&self, _: Option<&str>) -> Option<(&'static str, String)> {
        None
    }
    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn initial_message(
        &self,
        api_key: Option<&str>,
        params: &ListenParams,
        _: u8,
    ) -> Option<Message> {
        Some(Message::Text(json!({
            "type": "auth", "access_token": api_key.unwrap_or_default(),
            "language": params.languages.iter().map(|l| l.iso639_code().to_string()).collect::<Vec<_>>(),
            "context": { "dictionary_context": params.keywords },
        }).to_string().into()))
    }

    fn audio_to_message(&self, audio: bytes::Bytes) -> Message {
        if audio.is_empty() {
            return Message::Ping(bytes::Bytes::new());
        }
        let mut state = self.live.lock().unwrap_or_else(|e| e.into_inner());
        if state.packet_bytes == 0 {
            state.packet_bytes = audio.len().next_multiple_of(2);
        }
        let position = state.packets;
        let mut packets = Vec::new();
        let mut volumes = Vec::new();
        // Capture frames are uniform except for a possible final tail. Pad that
        // tail because Wispr requires one packet duration throughout a session.
        for chunk in audio.chunks(state.packet_bytes) {
            let mut packet = chunk.to_vec();
            packet.resize(state.packet_bytes, 0);
            let volume = (packet
                .chunks_exact(2)
                .map(|sample| {
                    (f64::from(i16::from_le_bytes([sample[0], sample[1]])) / 32768.0).powi(2)
                })
                .sum::<f64>()
                / (packet.len() / 2) as f64)
                .sqrt();
            packets.push(STANDARD.encode(packet));
            volumes.push(volume);
        }
        state.packets += packets.len();
        state.samples += audio.len() / 2;
        // Wispr's quickstart labels raw PCM16 packets as "wav" (no WAV header).
        Message::Text(
            json!({
                "type": "append", "position": position,
                "audio_packets": {
                    "packets": packets, "volumes": volumes,
                    "packet_duration": state.packet_bytes as f64 / 32_000.0,
                    "audio_encoding": "wav", "byte_encoding": "base64",
                }
            })
            .to_string()
            .into(),
        )
    }

    fn finalize_message(&self) -> Message {
        let state = self.live.lock().unwrap_or_else(|e| e.into_inner());
        Message::Text(
            json!({"type":"commit", "total_packets": state.packets})
                .to_string()
                .into(),
        )
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
            return vec![];
        };
        if value["status"] == "error" || value.get("error").is_some_and(|v| !v.is_null()) {
            return vec![StreamResponse::ErrorResponse {
                provider: "wisprflow".into(),
                error_code: None,
                error_message: "Wispr Flow rejected the transcription session.".into(),
            }];
        }
        if value["status"] != "text" {
            return vec![];
        }
        let Some(text) = value.pointer("/body/text").and_then(|v| v.as_str()) else {
            return vec![];
        };
        let is_final = value["final"].as_bool().unwrap_or(false);
        let duration =
            self.live.lock().unwrap_or_else(|e| e.into_inner()).samples as f64 / 16_000.0;
        vec![StreamResponse::TranscriptResponse {
            is_final,
            speech_final: is_final,
            from_finalize: is_final,
            start: 0.0,
            duration,
            channel: Channel {
                alternatives: vec![Alternatives {
                    transcript: text.to_string(),
                    words: if text.is_empty() {
                        vec![]
                    } else {
                        vec![
                            crate::adapter::parsing::WordBuilder::new(text)
                                .start(0.0)
                                .end(duration)
                                .build(),
                        ]
                    },
                    confidence: 1.0,
                    languages: value
                        .pointer("/body/detected_language")
                        .and_then(|v| v.as_str())
                        .map(|v| vec![v.to_string()])
                        .unwrap_or_default(),
                }],
            },
            metadata: Metadata::default(),
            channel_index: vec![0, 1],
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn channel_sessions_have_independent_packet_counts() {
        let adapter = WisprFlowAdapter::default();
        let other = adapter.fork_session();
        adapter.audio_to_message(vec![0; 3200].into());
        let commit = |a: &WisprFlowAdapter| {
            serde_json::from_str::<serde_json::Value>(&a.finalize_message().into_text().unwrap())
                .unwrap()
        };
        assert_eq!(commit(&adapter)["total_packets"], 1);
        assert_eq!(commit(&other)["total_packets"], 0);
    }
    #[test]
    fn pads_the_last_packet_to_preserve_the_session_duration() {
        let adapter = WisprFlowAdapter::default();
        adapter.audio_to_message(vec![0; 3200].into());
        let tail = adapter.audio_to_message(vec![1; 600].into());
        let tail: serde_json::Value = serde_json::from_str(&tail.into_text().unwrap()).unwrap();
        assert_eq!(tail["position"], 1);
        assert_eq!(tail["audio_packets"]["packet_duration"], 0.1);
        let bytes = STANDARD
            .decode(tail["audio_packets"]["packets"][0].as_str().unwrap())
            .unwrap();
        assert_eq!(bytes.len(), 3200);
        assert_eq!(&bytes[..600], &[1; 600]);
        assert!(bytes[600..].iter().all(|b| *b == 0));
    }
    #[tokio::test]
    async fn validates_endpoint_and_encodes_authentication() {
        let adapter = WisprFlowAdapter::default();
        assert!(
            adapter
                .build_ws_url_with_api_key("invalid", &ListenParams::default(), 1, Some("key"))
                .await
                .is_none()
        );
        let url = adapter
            .build_ws_url_with_api_key("", &ListenParams::default(), 1, Some("key&value"))
            .await
            .unwrap();
        assert_eq!(url.scheme(), "wss");
        assert_eq!(url.path(), "/api/v1/dash/ws");
        assert_eq!(url.query_pairs().next().unwrap().1, "Bearer key&value");
    }
    #[test]
    fn distinguishes_partial_final_and_error_responses() {
        let adapter = WisprFlowAdapter::default();
        for final_value in [false, true] {
            let responses = adapter.parse_response(
                &json!({"status":"text", "final":final_value, "body":{"text":"Hello."}})
                    .to_string(),
            );
            assert!(
                matches!(&responses[0], StreamResponse::TranscriptResponse {is_final, from_finalize, ..} if *is_final == final_value && *from_finalize == final_value)
            );
        }
        assert!(adapter.parse_response("invalid").is_empty());
        assert!(matches!(
            &adapter.parse_response(r#"{"status":"error"}"#)[0],
            StreamResponse::ErrorResponse { .. }
        ));
    }
}

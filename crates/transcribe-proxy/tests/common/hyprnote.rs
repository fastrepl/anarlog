use std::net::SocketAddr;

use super::{
    Direction, MockServerHandle, MockUpstreamConfig, WsMessage, WsRecording,
    start_split_mock_server_with_config,
};

const STEREO_TEST_AUDIO_FRAME: [u8; 8] = [1, 0, 9, 0, 2, 0, 10, 0];
const MIC_TEST_AUDIO: [u8; 4] = [1, 0, 2, 0];
const SPK_TEST_AUDIO: [u8; 4] = [9, 0, 10, 0];

#[derive(Debug)]
pub struct TranscriptEvent {
    pub text: String,
    pub channel_index: serde_json::Value,
    pub from_finalize: bool,
}

impl TranscriptEvent {
    pub fn matches(
        &self,
        text: &str,
        channel: usize,
        channels: usize,
        from_finalize: bool,
    ) -> bool {
        self.text == text
            && self.channel_index == serde_json::json!([channel, channels])
            && self.from_finalize == from_finalize
    }
}

pub async fn start_split_mock_ws(recordings: [WsRecording; 2]) -> MockServerHandle {
    start_split_mock_server_with_config(
        recordings[0].clone(),
        recordings[1].clone(),
        MockUpstreamConfig::default().use_timing(true),
        MIC_TEST_AUDIO.to_vec(),
        SPK_TEST_AUDIO.to_vec(),
    )
    .await
    .expect("failed to start split mock ws server")
}

pub fn split_test_audio_frame() -> Vec<u8> {
    STEREO_TEST_AUDIO_FRAME.to_vec()
}

pub fn soniox_finalize_message(text: &str) -> String {
    serde_json::json!({
        "tokens": [
            {
                "text": text,
                "start_ms": 0,
                "end_ms": 100,
                "confidence": 1.0,
                "is_final": true
            },
            {
                "text": "<fin>",
                "is_final": true
            }
        ],
        "finished": true
    })
    .to_string()
}

pub fn soniox_finalize_recording(
    text: &str,
    text_timestamp_ms: u64,
    close_timestamp_ms: u64,
    close_reason: &str,
) -> WsRecording {
    WsRecording {
        messages: vec![
            WsMessage::text(
                Direction::ServerToClient,
                text_timestamp_ms,
                soniox_finalize_message(text),
            ),
            WsMessage::close(
                Direction::ServerToClient,
                close_timestamp_ms,
                1000,
                close_reason,
            ),
        ],
    }
}

pub fn soniox_partial_recording(
    text: &str,
    text_timestamp_ms: u64,
    close_timestamp_ms: u64,
    close_reason: &str,
) -> WsRecording {
    WsRecording {
        messages: vec![
            WsMessage::text(
                Direction::ServerToClient,
                text_timestamp_ms,
                soniox_partial_message(text),
            ),
            WsMessage::close(
                Direction::ServerToClient,
                close_timestamp_ms,
                1000,
                close_reason,
            ),
        ],
    }
}

pub fn soniox_error_recording(
    error_message: &str,
    text_timestamp_ms: u64,
    close_timestamp_ms: u64,
) -> WsRecording {
    WsRecording {
        messages: vec![
            WsMessage::text(
                Direction::ServerToClient,
                text_timestamp_ms,
                serde_json::json!({
                    "error_code": 503,
                    "error_message": error_message,
                })
                .to_string(),
            ),
            WsMessage::close(Direction::ServerToClient, close_timestamp_ms, 1000, "error"),
        ],
    }
}

pub fn close_only_recording(timestamp_ms: u64, code: u16, reason: &str) -> WsRecording {
    WsRecording {
        messages: vec![WsMessage::close(
            Direction::ServerToClient,
            timestamp_ms,
            code,
            reason,
        )],
    }
}

pub fn transcript_events(messages: &[serde_json::Value]) -> Vec<TranscriptEvent> {
    messages
        .iter()
        .filter(|message| message["type"] == "Results")
        .map(|message| TranscriptEvent {
            text: message["channel"]["alternatives"][0]["transcript"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            channel_index: message["channel_index"].clone(),
            from_finalize: message["from_finalize"].as_bool().unwrap_or(false),
        })
        .collect()
}

pub fn terminal_finalize_count(messages: &[serde_json::Value]) -> usize {
    messages
        .iter()
        .filter(|message| message["type"] == "Results" && message["from_finalize"] == true)
        .count()
}

pub fn stereo_listen_url(addr: SocketAddr, query: &str) -> String {
    format!("ws://{addr}/listen?provider=hyprnote&sample_rate=16000&channels=2&{query}")
}

fn soniox_partial_message(text: &str) -> String {
    serde_json::json!({
        "tokens": [
            {
                "text": text,
                "start_ms": 0,
                "end_ms": 100,
                "confidence": 1.0,
                "is_final": false
            }
        ],
        "finished": false
    })
    .to_string()
}

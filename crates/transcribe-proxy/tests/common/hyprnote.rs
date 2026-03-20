use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use futures_util::StreamExt;
use owhisper_client::hypr_ws_client;
use owhisper_client::{
    BatchClient, BatchSttAdapter, DeepgramAdapter, HyprnoteAdapter, ListenClient,
};
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use owhisper_interface::{ControlMessage, ListenParams, MixedMessage};

use super::{
    Direction, MockServerHandle, MockUpstreamConfig, WsMessage, WsRecording, connect_to_url,
    start_mock_server_with_config, start_split_mock_server_with_config,
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

#[derive(Debug)]
pub struct ClientStreamResult {
    pub responses: Vec<StreamResponse>,
    pub terminal_error: Option<String>,
}

fn is_mock_trailing_disconnect(error: &hypr_ws_client::Error) -> bool {
    format!("{error:?}").contains("ResetWithoutClosingHandshake")
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

pub async fn start_mock_ws() -> MockServerHandle {
    start_mock_server_with_config(mock_recording(), MockUpstreamConfig::default())
        .await
        .expect("failed to start mock ws server")
}

pub fn english() -> Vec<hypr_language::Language> {
    vec![hypr_language::ISO639::En.into()]
}

pub fn split_test_audio_frame() -> Vec<u8> {
    STEREO_TEST_AUDIO_FRAME.to_vec()
}

pub fn single_response_recording(response: &StreamResponse) -> WsRecording {
    let mut recording = WsRecording::default();
    recording.push(WsMessage::text(
        Direction::ServerToClient,
        0,
        serde_json::to_string(response).expect("stream response should serialize"),
    ));
    recording.push(WsMessage::close(
        Direction::ServerToClient,
        1,
        1000,
        "normal",
    ));
    recording
}

pub fn sample_response(transcript: &str) -> StreamResponse {
    StreamResponse::TranscriptResponse {
        start: 0.0,
        duration: 0.0,
        is_final: true,
        speech_final: true,
        from_finalize: false,
        channel: Channel {
            alternatives: vec![Alternatives {
                transcript: transcript.to_string(),
                words: vec![],
                confidence: 1.0,
                languages: vec![],
            }],
        },
        metadata: Metadata::default(),
        channel_index: vec![0, 1],
    }
}

pub async fn send_streaming(addr: SocketAddr, query: &str) {
    let url = format!(
        "ws://{addr}/listen?provider=hyprnote&encoding=linear16&sample_rate=16000&channels=1&{query}"
    );
    let mut ws = connect_to_url(&url).await;
    let _ = ws.close(None).await;
}

pub async fn send_streaming_via_client(
    addr: SocketAddr,
    model: &str,
    languages: Vec<hypr_language::Language>,
) {
    let client = hyprnote_listen_client(addr, model, languages).await;

    let _ = client.from_realtime_audio(test_client_outbound()).await;
}

pub async fn collect_streaming_via_client(
    addr: SocketAddr,
    model: &str,
    languages: Vec<hypr_language::Language>,
    timeout: Duration,
) -> Vec<StreamResponse> {
    let result = collect_streaming_via_client_result(addr, model, languages, timeout).await;

    if let Some(error) = result.terminal_error {
        panic!("client stream should not terminate with an error: {error}");
    }

    result.responses
}

pub async fn collect_streaming_via_client_result(
    addr: SocketAddr,
    model: &str,
    languages: Vec<hypr_language::Language>,
    timeout: Duration,
) -> ClientStreamResult {
    let client = hyprnote_listen_client(addr, model, languages).await;

    let (stream, _handle) = client
        .from_realtime_audio(test_client_outbound())
        .await
        .expect("hyprnote streaming client should connect");

    futures_util::pin_mut!(stream);

    let mut responses = Vec::new();
    loop {
        let next = tokio::time::timeout(timeout, stream.next())
            .await
            .expect("timed out waiting for client stream item");

        match next {
            Some(Ok(response)) => responses.push(response),
            Some(Err(error)) => {
                if !responses.is_empty() && is_mock_trailing_disconnect(&error) {
                    break;
                }
                return ClientStreamResult {
                    responses,
                    terminal_error: Some(format!("display: {error}; debug: {error:?}")),
                };
            }
            None => break,
        }
    }

    ClientStreamResult {
        responses,
        terminal_error: None,
    }
}

pub async fn send_batch(addr: SocketAddr, query: &str) {
    let resp = reqwest::Client::new()
        .post(format!("http://{addr}/listen?provider=hyprnote&{query}"))
        .header("content-type", "audio/wav")
        .body(vec![1u8, 2, 3])
        .send()
        .await
        .expect("failed to send batch request");
    assert!(
        resp.status().is_success(),
        "batch request failed: {}",
        resp.status()
    );
}

pub async fn send_batch_via_hyprnote_client(
    addr: SocketAddr,
    model: &str,
    languages: Vec<hypr_language::Language>,
) -> owhisper_interface::batch::Response {
    batch_client::<HyprnoteAdapter>(addr, model, languages)
        .transcribe_file(hypr_data::english_1::AUDIO_PATH)
        .await
        .expect("hyprnote batch request should succeed")
}

pub async fn send_batch_via_deepgram_client(
    addr: SocketAddr,
    model: &str,
    languages: Vec<hypr_language::Language>,
) -> owhisper_interface::batch::Response {
    batch_client::<DeepgramAdapter>(addr, model, languages)
        .transcribe_file(hypr_data::english_1::AUDIO_PATH)
        .await
        .expect("deepgram passthrough batch request should succeed")
}

pub fn batch_upstream_url(addr: SocketAddr) -> String {
    format!("http://{addr}/v1")
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

pub fn soniox_finalize_ws_message(text: &str, timestamp_ms: u64) -> WsMessage {
    WsMessage::text(
        Direction::ServerToClient,
        timestamp_ms,
        soniox_finalize_message(text),
    )
}

pub fn soniox_finalize_recording(
    text: &str,
    text_timestamp_ms: u64,
    close_timestamp_ms: u64,
    close_reason: &str,
) -> WsRecording {
    WsRecording {
        messages: vec![
            soniox_finalize_ws_message(text, text_timestamp_ms),
            WsMessage::close(
                Direction::ServerToClient,
                close_timestamp_ms,
                1000,
                close_reason,
            ),
        ],
    }
}

pub fn soniox_partial_ws_message(text: &str, timestamp_ms: u64) -> WsMessage {
    WsMessage::text(
        Direction::ServerToClient,
        timestamp_ms,
        soniox_partial_message(text),
    )
}

pub fn soniox_partial_recording(
    text: &str,
    text_timestamp_ms: u64,
    close_timestamp_ms: u64,
    close_reason: &str,
) -> WsRecording {
    WsRecording {
        messages: vec![
            soniox_partial_ws_message(text, text_timestamp_ms),
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

async fn hyprnote_listen_client(
    addr: SocketAddr,
    model: &str,
    languages: Vec<hypr_language::Language>,
) -> ListenClient<HyprnoteAdapter> {
    ListenClient::builder()
        .adapter::<HyprnoteAdapter>()
        .api_base(format!("http://{addr}/listen"))
        .params(ListenParams {
            model: Some(model.to_string()),
            languages,
            sample_rate: 16000,
            channels: 1,
            ..Default::default()
        })
        .build_single()
        .await
}

fn test_client_outbound() -> impl futures_util::Stream<Item = MixedMessage<Bytes, ControlMessage>> {
    tokio_stream::iter(vec![
        MixedMessage::Audio(Bytes::from_static(&[0u8, 1, 2, 3])),
        MixedMessage::Control(ControlMessage::Finalize),
    ])
}

fn batch_client<A: BatchSttAdapter>(
    addr: SocketAddr,
    model: &str,
    languages: Vec<hypr_language::Language>,
) -> BatchClient<A> {
    BatchClient::<A>::builder()
        .api_base(format!("http://{addr}/stt"))
        .api_key("test-access-token")
        .params(ListenParams {
            model: Some(model.to_string()),
            languages,
            ..Default::default()
        })
        .build()
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

fn mock_recording() -> WsRecording {
    let mut recording = WsRecording::default();
    recording.push(WsMessage::text(
        Direction::ServerToClient,
        0,
        r#"{"type":"Results"}"#,
    ));
    recording.push(WsMessage::close(
        Direction::ServerToClient,
        1,
        1000,
        "normal",
    ));
    recording
}

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use common::{
    CloseInfo, Direction, MessageKind, MockUpstreamConfig, TranscriptEvent, WsMessage, WsRecording,
    close_only_recording, connect_to_url, soniox_error_recording, soniox_finalize_message,
    soniox_finalize_recording, soniox_partial_recording, split_test_audio_frame,
    start_mock_batch_upstream, start_mock_server_with_config, start_proxy, start_proxy_under_stt,
    start_split_mock_ws, stereo_listen_url, terminal_finalize_count, transcript_events, wait_for,
};
use futures_util::{SinkExt, StreamExt};
use owhisper_client::{BatchClient, DeepgramAdapter, HyprnoteAdapter, ListenClient, Provider};
use owhisper_interface::{ControlMessage, ListenParams, MixedMessage};
use tokio_tungstenite::tungstenite::Message;

const TIMEOUT: Duration = Duration::from_secs(2);

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

async fn start_mock_ws() -> common::MockServerHandle {
    start_mock_server_with_config(mock_recording(), MockUpstreamConfig::default())
        .await
        .expect("failed to start mock ws server")
}

async fn send_streaming(addr: SocketAddr, query: &str) {
    let url = format!(
        "ws://{addr}/listen?provider=hyprnote&encoding=linear16&sample_rate=16000&channels=1&{query}"
    );
    let mut ws = connect_to_url(&url).await;
    let _ = ws.close(None).await;
}

struct DualStreamingResult {
    messages: Vec<serde_json::Value>,
    close_info: CloseInfo,
    soniox_request_count: usize,
}

async fn send_streaming_dual(
    addr: SocketAddr,
    query: &str,
    send_finalize: bool,
) -> (Vec<serde_json::Value>, CloseInfo) {
    let mut ws = connect_to_url(&stereo_listen_url(addr, query)).await;
    ws.send(Message::Binary(split_test_audio_frame().into()))
        .await
        .expect("failed to send audio");
    if send_finalize {
        ws.send(Message::Text(
            serde_json::to_string(&ControlMessage::Finalize)
                .unwrap()
                .into(),
        ))
        .await
        .expect("failed to send finalize");
    }

    let mut messages = Vec::new();
    let mut close_info = None;

    loop {
        let next = tokio::time::timeout(TIMEOUT, ws.next())
            .await
            .expect("timed out waiting for proxy response");
        match next {
            Some(Ok(Message::Text(text))) => {
                messages.push(serde_json::from_str(&text).expect("proxy returned invalid JSON"));
            }
            Some(Ok(Message::Close(frame))) => {
                close_info = frame.map(|f| (f.code.into(), f.reason.to_string()));
                break;
            }
            Some(Ok(_)) => {}
            Some(Err(e)) => panic!("websocket error: {e:?}"),
            None => break,
        }
    }

    (messages, close_info)
}

async fn run_dual_soniox_case(
    recordings: [WsRecording; 2],
    send_finalize: bool,
) -> DualStreamingResult {
    let soniox_mock = start_split_mock_ws(recordings).await;
    let deepgram_mock = start_mock_ws().await;
    let proxy = start_proxy(Some(&deepgram_mock.ws_url()), Some(&soniox_mock.ws_url())).await;
    let (messages, close_info) =
        send_streaming_dual(proxy, "model=cloud&language=en&language=ko", send_finalize).await;

    DualStreamingResult {
        messages,
        close_info,
        soniox_request_count: soniox_mock.captured_requests().len(),
    }
}

fn has_transcript(
    transcripts: &[TranscriptEvent],
    text: &str,
    channel: usize,
    from_finalize: bool,
) -> bool {
    transcripts
        .iter()
        .any(|event| event.matches(text, channel, 2, from_finalize))
}

fn has_soniox_error(messages: &[serde_json::Value], error_message: &str) -> bool {
    messages.iter().any(|message| {
        message["type"] == "Error"
            && message["provider"] == "soniox"
            && message["error_message"] == error_message
    })
}

fn last_message_is_soniox_error(messages: &[serde_json::Value], error_message: &str) -> bool {
    matches!(
        messages.last(),
        Some(message)
            if message["type"] == "Error"
                && message["provider"] == "soniox"
                && message["error_message"] == error_message
    )
}

fn transcript_message_index(
    messages: &[serde_json::Value],
    text: &str,
    channel: usize,
    from_finalize: bool,
) -> usize {
    messages
        .iter()
        .position(|message| {
            message["type"] == "Results"
                && message["channel"]["alternatives"][0]["transcript"] == text
                && message["channel_index"] == serde_json::json!([channel, 2])
                && message["from_finalize"] == from_finalize
        })
        .expect("expected transcript message")
}

fn error_message_index(messages: &[serde_json::Value], error_message: &str) -> usize {
    messages
        .iter()
        .position(|message| {
            message["type"] == "Error"
                && message["provider"] == "soniox"
                && message["error_message"] == error_message
        })
        .expect("expected soniox error message")
}

async fn send_streaming_via_client(
    addr: SocketAddr,
    model: &str,
    languages: Vec<hypr_language::Language>,
) {
    let client = ListenClient::builder()
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
        .await;

    let outbound = tokio_stream::iter(vec![
        MixedMessage::Audio(Bytes::from_static(&[0u8, 1, 2, 3])),
        MixedMessage::Control(ControlMessage::Finalize),
    ]);

    let _ = client.from_realtime_audio(outbound).await;
}

async fn send_batch(addr: SocketAddr, query: &str) {
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

async fn send_batch_via_hyprnote_client(
    addr: SocketAddr,
    model: &str,
    languages: Vec<hypr_language::Language>,
) -> owhisper_interface::batch::Response {
    BatchClient::<HyprnoteAdapter>::builder()
        .api_base(format!("http://{addr}/stt"))
        .api_key("test-access-token")
        .params(ListenParams {
            model: Some(model.to_string()),
            languages,
            ..Default::default()
        })
        .build()
        .transcribe_file(hypr_data::english_1::AUDIO_PATH)
        .await
        .expect("hyprnote batch request should succeed")
}

async fn send_batch_via_deepgram_client(
    addr: SocketAddr,
    model: &str,
    languages: Vec<hypr_language::Language>,
) -> owhisper_interface::batch::Response {
    BatchClient::<DeepgramAdapter>::builder()
        .api_base(format!("http://{addr}/stt"))
        .api_key("test-access-token")
        .params(ListenParams {
            model: Some(model.to_string()),
            languages,
            ..Default::default()
        })
        .build()
        .transcribe_file(hypr_data::english_1::AUDIO_PATH)
        .await
        .expect("deepgram passthrough batch request should succeed")
}

fn batch_upstream_url(addr: SocketAddr) -> String {
    format!("http://{addr}/v1")
}

#[tokio::test]
async fn streaming_cloud_model_resolved_for_deepgram() {
    let mock = start_mock_ws().await;
    let proxy = start_proxy(Some(&mock.ws_url()), None).await;

    send_streaming(proxy, "model=cloud&language=en").await;
    let req = wait_for(TIMEOUT, || mock.captured_requests().first().cloned()).await;

    assert!(
        req.contains("model=nova-3"),
        "should resolve cloud -> nova-3 for en: {req}"
    );
    assert!(
        !req.contains("model=cloud"),
        "meta model should not leak upstream: {req}"
    );
}

#[tokio::test]
async fn streaming_cloud_model_removed_for_soniox() {
    let mock = start_mock_ws().await;
    let proxy = start_proxy(None, Some(&mock.ws_url())).await;

    send_streaming(proxy, "model=cloud&language=ko&language=en").await;
    let req = wait_for(TIMEOUT, || mock.captured_requests().first().cloned()).await;

    assert!(
        !req.contains("model=cloud"),
        "meta model should not leak upstream: {req}"
    );
    assert!(
        !req.contains("model="),
        "soniox should not receive explicit model for cloud: {req}"
    );
}

#[tokio::test]
async fn streaming_routing_selects_soniox_for_en_ko() {
    let dg_mock = start_mock_ws().await;
    let sox_mock = start_mock_ws().await;
    let proxy = start_proxy(Some(&dg_mock.ws_url()), Some(&sox_mock.ws_url())).await;

    send_streaming(proxy, "model=cloud&language=en&language=ko").await;
    let sox_req = wait_for(TIMEOUT, || sox_mock.captured_requests().first().cloned()).await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        dg_mock.captured_requests().is_empty(),
        "deepgram should not be selected for en+ko"
    );
    assert!(
        !sox_req.contains("model=cloud"),
        "meta model should not leak to soniox: {sox_req}"
    );
}

#[tokio::test]
async fn streaming_explicit_model_preserved_for_deepgram() {
    let mock = start_mock_ws().await;
    let proxy = start_proxy(Some(&mock.ws_url()), None).await;

    send_streaming(proxy, "model=nova-3&language=en").await;
    let req = wait_for(TIMEOUT, || mock.captured_requests().first().cloned()).await;

    assert!(
        req.contains("model=nova-3"),
        "explicit model should be preserved: {req}"
    );
}

#[tokio::test]
async fn streaming_client_adapter_resolves_cloud_model() {
    let mock = start_mock_ws().await;
    let proxy = start_proxy(Some(&mock.ws_url()), None).await;

    send_streaming_via_client(proxy, "cloud", vec![hypr_language::ISO639::En.into()]).await;
    let req = wait_for(TIMEOUT, || mock.captured_requests().first().cloned()).await;

    assert!(
        req.contains("model=nova-3"),
        "should resolve cloud -> nova-3 for en: {req}"
    );
    assert!(
        !req.contains("model=cloud"),
        "meta model should not leak upstream: {req}"
    );
    assert!(
        req.contains("sample_rate=16000") && req.contains("channels=1"),
        "listen params should reach upstream: {req}"
    );
}

#[tokio::test]
async fn batch_cloud_model_resolved_for_deepgram() {
    let batch = start_mock_batch_upstream().await;
    let upstream_url = batch_upstream_url(batch.addr);
    let proxy = start_proxy(Some(&upstream_url), None).await;

    send_batch(proxy, "model=cloud&language=en").await;
    let query = wait_for(TIMEOUT, || batch.first_query()).await;

    assert!(
        query.contains("model=nova-3"),
        "should resolve cloud -> nova-3 for en: {query}"
    );
    assert!(
        !query.contains("model=cloud"),
        "meta model should not leak upstream: {query}"
    );
}

#[tokio::test]
async fn batch_explicit_model_preserved_for_deepgram() {
    let batch = start_mock_batch_upstream().await;
    let upstream_url = batch_upstream_url(batch.addr);
    let proxy = start_proxy(Some(&upstream_url), None).await;

    send_batch(proxy, "model=nova-3&language=en").await;
    let query = wait_for(TIMEOUT, || batch.first_query()).await;

    assert!(
        query.contains("model=nova-3"),
        "explicit model should be preserved: {query}"
    );
}

#[tokio::test]
async fn batch_client_hyprnote_adapter_uses_proxy_sync_path_under_stt() {
    let batch = start_mock_batch_upstream().await;
    let upstream_url = batch_upstream_url(batch.addr);
    let proxy = start_proxy_under_stt(Provider::Deepgram, Some(&upstream_url), None).await;

    let response =
        send_batch_via_hyprnote_client(proxy, "cloud", vec![hypr_language::ISO639::En.into()])
            .await;
    let query = wait_for(TIMEOUT, || batch.first_query()).await;

    let transcript = response
        .results
        .channels
        .first()
        .and_then(|channel| channel.alternatives.first())
        .map(|alt| alt.transcript.as_str())
        .unwrap_or("");

    assert_eq!(
        transcript, "ok",
        "proxy response should round-trip upstream batch payload"
    );
    assert!(
        query.contains("model=nova-3"),
        "hyprnote sync batch should resolve cloud -> nova-3 before upstream: {query}"
    );
    assert!(
        !query.contains("model=cloud"),
        "meta model should not leak upstream: {query}"
    );
}

#[tokio::test]
async fn batch_client_deepgram_adapter_passthrough_uses_provider_query_under_stt() {
    let batch = start_mock_batch_upstream().await;
    let upstream_url = batch_upstream_url(batch.addr);
    let proxy = start_proxy_under_stt(Provider::Soniox, Some(&upstream_url), None).await;

    let response =
        send_batch_via_deepgram_client(proxy, "nova-2", vec![hypr_language::ISO639::En.into()])
            .await;
    let query = wait_for(TIMEOUT, || batch.first_query()).await;

    let transcript = response
        .results
        .channels
        .first()
        .and_then(|channel| channel.alternatives.first())
        .map(|alt| alt.transcript.as_str())
        .unwrap_or("");

    assert_eq!(
        transcript, "ok",
        "passthrough batch should return upstream response"
    );
    assert!(
        query.contains("model=nova-2"),
        "passthrough batch should preserve the direct-provider request shape: {query}"
    );
}

#[tokio::test]
async fn streaming_dual_soniox_emits_both_channel_finals_but_only_one_terminal_finalize() {
    let mic_recording = soniox_finalize_recording("Mic done", 20, 30, "mic done");
    let spk_recording = soniox_finalize_recording("Speaker done", 120, 140, "speaker done");
    let result = run_dual_soniox_case([mic_recording, spk_recording], true).await;

    assert_eq!(
        result.soniox_request_count, 2,
        "split mode should open two Soniox upstream sessions"
    );

    assert_eq!(
        terminal_finalize_count(&result.messages),
        1,
        "proxy should expose a single terminal finalize to downstream clients"
    );

    let transcripts = transcript_events(&result.messages);

    assert!(
        has_transcript(&transcripts, "Mic done", 0, false),
        "mic finalize should be preserved but not terminate the downstream session: {transcripts:?}"
    );
    assert!(
        has_transcript(&transcripts, "Speaker done", 1, true),
        "speaker finalize should terminate the downstream session once both channels are done: {transcripts:?}"
    );
    assert_eq!(result.close_info, Some((1000, "speaker done".to_string())));
}

#[tokio::test]
async fn streaming_dual_soniox_mic_finalize_speaker_only_closes_still_emits_terminal_finalize() {
    let mic_recording = soniox_finalize_recording("Mic done", 20, 200, "mic done");
    let spk_recording = WsRecording {
        messages: vec![WsMessage::close(
            Direction::ServerToClient,
            120,
            1000,
            "speaker done",
        )],
    };
    let result = run_dual_soniox_case([mic_recording, spk_recording], true).await;

    assert_eq!(result.soniox_request_count, 2);
    assert_eq!(terminal_finalize_count(&result.messages), 1);

    let transcripts = transcript_events(&result.messages);
    assert!(
        has_transcript(&transcripts, "Mic done", 0, true),
        "mic finalize should become terminal when the speaker channel only closes: {transcripts:?}"
    );
    assert!(
        matches!(result.close_info, Some((1000, _))),
        "session should still close normally after the sibling regular-results path: {:?}",
        result.close_info
    );
}

#[tokio::test]
async fn streaming_dual_soniox_speaker_finalize_mic_only_closes_still_emits_terminal_finalize() {
    let mic_recording = WsRecording {
        messages: vec![WsMessage::close(
            Direction::ServerToClient,
            120,
            1000,
            "mic done",
        )],
    };
    let spk_recording = soniox_finalize_recording("Speaker done", 20, 200, "speaker done");
    let result = run_dual_soniox_case([mic_recording, spk_recording], true).await;

    assert_eq!(result.soniox_request_count, 2);
    assert_eq!(terminal_finalize_count(&result.messages), 1);

    let transcripts = transcript_events(&result.messages);
    assert!(
        has_transcript(&transcripts, "Speaker done", 1, true),
        "speaker finalize should become terminal when the mic channel only closes: {transcripts:?}"
    );
    assert_eq!(result.close_info, Some((1000, "speaker done".to_string())));
}

#[tokio::test]
async fn streaming_dual_soniox_finalize_then_other_channel_regular_results_then_close_keeps_one_terminal_finalize()
 {
    let mic_recording = soniox_finalize_recording("Mic done", 20, 220, "mic done");
    let spk_recording = soniox_partial_recording("Speaker partial", 120, 180, "speaker done");
    let result = run_dual_soniox_case([mic_recording, spk_recording], true).await;

    assert_eq!(result.soniox_request_count, 2);
    assert_eq!(terminal_finalize_count(&result.messages), 1);

    let transcripts = transcript_events(&result.messages);
    assert!(
        has_transcript(&transcripts, "Mic done", 0, true),
        "buffered finalize should be released once the sibling closes: {transcripts:?}"
    );
    assert!(
        has_transcript(&transcripts, "Speaker partial", 1, false),
        "non-final sibling results should still flow while finalize is pending: {transcripts:?}"
    );
    assert!(
        transcript_message_index(&result.messages, "Speaker partial", 1, false)
            < transcript_message_index(&result.messages, "Mic done", 0, true),
        "sibling non-final traffic should arrive before the delayed terminal finalize: {:?}",
        result.messages
    );
    assert_eq!(result.close_info, Some((1000, "mic done".to_string())));
}

#[tokio::test]
async fn streaming_dual_soniox_no_channel_finalize_closes_without_terminal_finalize() {
    let mic_recording = WsRecording {
        messages: vec![WsMessage::close(
            Direction::ServerToClient,
            80,
            1000,
            "mic done",
        )],
    };
    let spk_recording = WsRecording {
        messages: vec![WsMessage::close(
            Direction::ServerToClient,
            120,
            1000,
            "speaker done",
        )],
    };
    let result = run_dual_soniox_case([mic_recording, spk_recording], false).await;

    assert_eq!(result.soniox_request_count, 2);
    assert_eq!(terminal_finalize_count(&result.messages), 0);
    assert_eq!(result.close_info, Some((1000, "speaker done".to_string())));
}

#[tokio::test]
async fn streaming_dual_soniox_forwards_error_before_non_normal_close() {
    let error_message = "Cannot continue request (code 1). Please restart the request.";
    let mic_recording = soniox_error_recording(error_message, 20, 250);
    let spk_recording = WsRecording {
        messages: vec![WsMessage::close(
            Direction::ServerToClient,
            200,
            1000,
            "speaker done",
        )],
    };
    let result = run_dual_soniox_case([mic_recording, spk_recording], false).await;

    assert!(
        has_soniox_error(&result.messages, error_message),
        "proxy should forward transformed Soniox errors before closing: {:?}",
        result.messages
    );
    assert!(
        last_message_is_soniox_error(&result.messages, error_message),
        "provider error payload should be the final text message before close: {:?}",
        result.messages
    );

    let (code, reason) = result
        .close_info
        .expect("proxy should close the downstream websocket");
    assert_ne!(
        code, 1000,
        "provider errors should not map to a normal close"
    );
    assert!(
        reason.contains(error_message),
        "close reason should carry the provider error: {reason}"
    );
}

#[tokio::test]
async fn streaming_dual_soniox_pending_finalize_is_delivered_before_error_close() {
    let error_message = "Cannot continue request (code 1). Please restart the request.";
    let mic_recording = soniox_finalize_recording("Mic done", 20, 250, "mic done");
    let spk_recording = soniox_error_recording(error_message, 120, 250);
    let result = run_dual_soniox_case([mic_recording, spk_recording], false).await;

    let transcripts = transcript_events(&result.messages);
    assert!(
        has_transcript(&transcripts, "Mic done", 0, false),
        "buffered finalize-bearing transcript should still be delivered before the error: {transcripts:?}"
    );
    assert!(
        has_soniox_error(&result.messages, error_message),
        "proxy should still forward the provider error payload: {:?}",
        result.messages
    );
    assert!(
        last_message_is_soniox_error(&result.messages, error_message),
        "provider error payload should be the final text message before close: {:?}",
        result.messages
    );
    assert!(
        transcript_message_index(&result.messages, "Mic done", 0, false)
            < error_message_index(&result.messages, error_message),
        "pending finalize should be flushed before the provider error payload: {:?}",
        result.messages
    );
    let (code, reason) = result
        .close_info
        .expect("proxy should close the downstream websocket");
    assert_ne!(code, 1000);
    assert!(reason.contains(error_message));
}

#[tokio::test]
async fn streaming_dual_soniox_pending_finalize_is_downgraded_before_non_normal_close() {
    let mic_recording = soniox_finalize_recording("Mic done", 20, 250, "mic done");
    let spk_recording = close_only_recording(120, 1011, "speaker_failed");
    let result = run_dual_soniox_case([mic_recording, spk_recording], false).await;

    let transcripts = transcript_events(&result.messages);
    assert!(
        has_transcript(&transcripts, "Mic done", 0, false),
        "pending finalize should be flushed as non-terminal before abnormal close: {transcripts:?}"
    );
    assert_eq!(terminal_finalize_count(&result.messages), 0);
    assert_eq!(
        result.close_info,
        Some((1011, "speaker_failed".to_string()))
    );
}

#[tokio::test]
async fn streaming_dual_soniox_later_finalize_replaces_earlier_pending_finalize() {
    let mic_recording = WsRecording {
        messages: vec![
            WsMessage::text(
                Direction::ServerToClient,
                20,
                soniox_finalize_message("Mic first"),
            ),
            WsMessage::text(
                Direction::ServerToClient,
                80,
                soniox_finalize_message("Mic second"),
            ),
            WsMessage::close(Direction::ServerToClient, 200, 1000, "mic done"),
        ],
    };
    let spk_recording = close_only_recording(140, 1000, "speaker done");
    let result = run_dual_soniox_case([mic_recording, spk_recording], true).await;

    let transcripts = transcript_events(&result.messages);
    assert!(
        transcripts
            .iter()
            .any(|event| event.text == "Mic first" && !event.from_finalize),
        "older pending finalize should be flushed as non-terminal when replaced: {transcripts:?}"
    );
    assert!(
        has_transcript(&transcripts, "Mic second", 0, true),
        "newer finalize should become the terminal finalize once sibling closes: {transcripts:?}"
    );
    assert_eq!(terminal_finalize_count(&result.messages), 1);
    assert!(
        matches!(result.close_info, Some((1000, _))),
        "session should still close normally: {:?}",
        result.close_info
    );
}

#[tokio::test]
async fn streaming_dual_soniox_keeps_pending_finalize_buffered_until_emitting_channel_closes() {
    let mic_recording = WsRecording {
        messages: vec![
            WsMessage::text(
                Direction::ServerToClient,
                20,
                soniox_finalize_message("Mic first"),
            ),
            WsMessage::text(
                Direction::ServerToClient,
                180,
                soniox_finalize_message("Mic second"),
            ),
            WsMessage::close(Direction::ServerToClient, 240, 1000, "mic done"),
        ],
    };
    let spk_recording = close_only_recording(120, 1000, "speaker done");
    let result = run_dual_soniox_case([mic_recording, spk_recording], true).await;

    let transcripts = transcript_events(&result.messages);
    assert_eq!(terminal_finalize_count(&result.messages), 1);
    assert!(
        has_transcript(&transcripts, "Mic first", 0, false),
        "the earlier finalize should stay buffered until the mic channel closes and then downgrade when replaced: {transcripts:?}"
    );
    assert!(
        has_transcript(&transcripts, "Mic second", 0, true),
        "the later finalize should become terminal only when the emitting channel closes: {transcripts:?}"
    );
}

#[tokio::test]
async fn streaming_dual_soniox_keeps_replacing_pending_finalize_until_emitting_channel_closes() {
    let mic_recording = soniox_finalize_recording("Mic done", 20, 220, "mic done");
    let spk_recording = WsRecording {
        messages: vec![
            WsMessage::text(
                Direction::ServerToClient,
                120,
                soniox_finalize_message("Speaker first"),
            ),
            WsMessage::text(
                Direction::ServerToClient,
                180,
                soniox_finalize_message("Speaker second"),
            ),
            WsMessage::close(Direction::ServerToClient, 240, 1000, "speaker done"),
        ],
    };
    let result = run_dual_soniox_case([mic_recording, spk_recording], true).await;

    let transcripts = transcript_events(&result.messages);
    assert_eq!(terminal_finalize_count(&result.messages), 1);
    assert!(
        has_transcript(&transcripts, "Mic done", 0, false),
        "the earlier pending finalize should be downgraded once the later channel replaces it: {transcripts:?}"
    );
    assert!(
        has_transcript(&transcripts, "Speaker first", 1, false),
        "earlier finalize-bearing updates from the still-open terminal channel should stay non-terminal: {transcripts:?}"
    );
    assert!(
        has_transcript(&transcripts, "Speaker second", 1, true),
        "the latest finalize from the still-open emitting channel should become terminal only when that channel closes: {transcripts:?}"
    );
}

#[tokio::test]
async fn streaming_dual_split_rejects_invalid_stereo_frame_alignment() {
    let soniox_mock = start_split_mock_ws([WsRecording::default(), WsRecording::default()]).await;
    let deepgram_mock = start_mock_ws().await;
    let proxy = start_proxy(Some(&deepgram_mock.ws_url()), Some(&soniox_mock.ws_url())).await;
    let mut ws = connect_to_url(&stereo_listen_url(
        proxy,
        "model=cloud&language=en&language=ko",
    ))
    .await;
    ws.send(Message::Binary(vec![0u8, 1].into()))
        .await
        .expect("failed to send malformed audio");

    let next = tokio::time::timeout(TIMEOUT, ws.next())
        .await
        .expect("timed out waiting for proxy close");

    match next {
        Some(Ok(Message::Close(Some(frame)))) => {
            let code: u16 = frame.code.into();
            assert_eq!(code, 1011);
            assert_eq!(frame.reason, "invalid_stereo_frame_alignment");
        }
        other => panic!("expected close frame for malformed split audio, got {other:?}"),
    }
}

#[tokio::test]
async fn streaming_dual_client_close_propagates_to_both_upstreams() {
    let mic_recording = WsRecording {
        messages: vec![
            WsMessage::text(
                Direction::ServerToClient,
                0,
                serde_json::json!({
                    "tokens": [{
                        "text": "Mic partial",
                        "start_ms": 0,
                        "end_ms": 100,
                        "confidence": 1.0,
                        "is_final": false
                    }],
                    "finished": false
                })
                .to_string(),
            ),
            WsMessage::close(Direction::ServerToClient, 300, 1000, "mic done"),
        ],
    };
    let spk_recording = WsRecording {
        messages: vec![
            WsMessage::text(
                Direction::ServerToClient,
                0,
                serde_json::json!({
                    "tokens": [{
                        "text": "Speaker partial",
                        "start_ms": 0,
                        "end_ms": 100,
                        "confidence": 1.0,
                        "is_final": false
                    }],
                    "finished": false
                })
                .to_string(),
            ),
            WsMessage::close(Direction::ServerToClient, 300, 1000, "speaker done"),
        ],
    };

    let soniox_mock = start_split_mock_ws([mic_recording, spk_recording]).await;
    let deepgram_mock = start_mock_ws().await;
    let proxy = start_proxy(Some(&deepgram_mock.ws_url()), Some(&soniox_mock.ws_url())).await;

    let mut ws = connect_to_url(&stereo_listen_url(
        proxy,
        "model=cloud&language=en&language=ko",
    ))
    .await;
    ws.send(Message::Binary(split_test_audio_frame().into()))
        .await
        .expect("failed to send audio");
    ws.send(Message::Text(
        serde_json::to_string(&ControlMessage::Finalize)
            .unwrap()
            .into(),
    ))
    .await
    .expect("failed to send finalize");

    let _ = tokio::time::timeout(TIMEOUT, ws.next())
        .await
        .expect("timed out waiting for first proxy response");

    ws.close(None)
        .await
        .expect("failed to close downstream client websocket");

    let client_messages = wait_for(TIMEOUT, || {
        let messages = soniox_mock.captured_client_messages();
        (messages
            .iter()
            .filter(|kind| matches!(kind, MessageKind::Close { .. }))
            .count()
            == 2)
            .then_some(messages)
    })
    .await;

    assert_eq!(
        client_messages
            .iter()
            .filter(|kind| matches!(kind, MessageKind::Close { .. }))
            .count(),
        2,
        "downstream close should propagate to both upstream split sessions"
    );
}

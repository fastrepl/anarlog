mod common;

use std::net::SocketAddr;
use std::time::Duration;

use common::{
    CloseInfo, MessageKind, TranscriptEvent, WsMessage, WsRecording, close_only_recording,
    collect_json_messages, connect_to_url, soniox_error_recording, soniox_finalize_recording,
    soniox_finalize_ws_message, soniox_partial_recording, soniox_partial_ws_message,
    split_test_audio_frame, start_mock_ws, start_proxy, start_split_mock_ws, stereo_listen_url,
    terminal_finalize_count, transcript_events,
};
use futures_util::{SinkExt, StreamExt};
use owhisper_interface::ControlMessage;
use tokio_tungstenite::tungstenite::Message;

const DUAL_QUERY: &str = "model=cloud&language=en&language=ko";
const TIMEOUT: Duration = Duration::from_secs(2);
const SPLIT_REQUEST_COUNT: usize = 2;

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
    collect_json_messages(ws, TIMEOUT).await
}

async fn run_dual_soniox_case(
    recordings: [WsRecording; 2],
    send_finalize: bool,
) -> DualStreamingResult {
    let soniox_mock = start_split_mock_ws(recordings).await;
    let deepgram_mock = start_mock_ws().await;
    let proxy = start_proxy(Some(&deepgram_mock.ws_url()), Some(&soniox_mock.ws_url())).await;
    let (messages, close_info) = send_streaming_dual(proxy, DUAL_QUERY, send_finalize).await;

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

fn assert_split_requests(result: &DualStreamingResult) {
    assert_eq!(
        result.soniox_request_count, SPLIT_REQUEST_COUNT,
        "split mode should open two Soniox upstream sessions"
    );
}

fn assert_close_info(result: &DualStreamingResult, expected: (u16, &str)) {
    assert_eq!(
        result.close_info,
        Some((expected.0, expected.1.to_string()))
    );
}

fn partial_then_close_recording(
    text: &str,
    text_timestamp_ms: u64,
    close_timestamp_ms: u64,
    close_reason: &str,
) -> WsRecording {
    WsRecording {
        messages: vec![
            soniox_partial_ws_message(text, text_timestamp_ms),
            WsMessage::close(
                common::Direction::ServerToClient,
                close_timestamp_ms,
                1000,
                close_reason,
            ),
        ],
    }
}

#[tokio::test]
async fn streaming_dual_soniox_emits_both_channel_finals_but_only_one_terminal_finalize() {
    let mic_recording = soniox_finalize_recording("Mic done", 20, 30, "mic done");
    let spk_recording = soniox_finalize_recording("Speaker done", 120, 140, "speaker done");
    let result = run_dual_soniox_case([mic_recording, spk_recording], true).await;

    assert_split_requests(&result);
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
    assert_close_info(&result, (1000, "speaker done"));
}

#[tokio::test]
async fn streaming_dual_soniox_mic_finalize_speaker_only_closes_still_emits_terminal_finalize() {
    let mic_recording = soniox_finalize_recording("Mic done", 20, 200, "mic done");
    let spk_recording = close_only_recording(120, 1000, "speaker done");
    let result = run_dual_soniox_case([mic_recording, spk_recording], true).await;

    assert_split_requests(&result);
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
    let mic_recording = close_only_recording(120, 1000, "mic done");
    let spk_recording = soniox_finalize_recording("Speaker done", 20, 200, "speaker done");
    let result = run_dual_soniox_case([mic_recording, spk_recording], true).await;

    assert_split_requests(&result);
    assert_eq!(terminal_finalize_count(&result.messages), 1);

    let transcripts = transcript_events(&result.messages);
    assert!(
        has_transcript(&transcripts, "Speaker done", 1, true),
        "speaker finalize should become terminal when the mic channel only closes: {transcripts:?}"
    );
    assert_close_info(&result, (1000, "speaker done"));
}

#[tokio::test]
async fn streaming_dual_soniox_finalize_then_other_channel_regular_results_then_close_keeps_one_terminal_finalize()
 {
    let mic_recording = soniox_finalize_recording("Mic done", 20, 220, "mic done");
    let spk_recording = soniox_partial_recording("Speaker partial", 120, 180, "speaker done");
    let result = run_dual_soniox_case([mic_recording, spk_recording], true).await;

    assert_split_requests(&result);
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
    assert_close_info(&result, (1000, "mic done"));
}

#[tokio::test]
async fn streaming_dual_soniox_no_channel_finalize_closes_without_terminal_finalize() {
    let mic_recording = close_only_recording(80, 1000, "mic done");
    let spk_recording = close_only_recording(120, 1000, "speaker done");
    let result = run_dual_soniox_case([mic_recording, spk_recording], false).await;

    assert_split_requests(&result);
    assert_eq!(terminal_finalize_count(&result.messages), 0);
    assert_close_info(&result, (1000, "speaker done"));
}

#[tokio::test]
async fn streaming_dual_soniox_forwards_error_before_non_normal_close() {
    let error_message = "Cannot continue request (code 1). Please restart the request.";
    let mic_recording = soniox_error_recording(error_message, 20, 250);
    let spk_recording = close_only_recording(200, 1000, "speaker done");
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
    assert_close_info(&result, (1011, "speaker_failed"));
}

#[tokio::test]
async fn streaming_dual_soniox_later_finalize_replaces_earlier_pending_finalize() {
    let mic_recording = WsRecording {
        messages: vec![
            soniox_finalize_ws_message("Mic first", 20),
            soniox_finalize_ws_message("Mic second", 80),
            WsMessage::close(common::Direction::ServerToClient, 200, 1000, "mic done"),
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
            soniox_finalize_ws_message("Mic first", 20),
            soniox_finalize_ws_message("Mic second", 180),
            WsMessage::close(common::Direction::ServerToClient, 240, 1000, "mic done"),
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
            soniox_finalize_ws_message("Speaker first", 120),
            soniox_finalize_ws_message("Speaker second", 180),
            WsMessage::close(common::Direction::ServerToClient, 240, 1000, "speaker done"),
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
    let mut ws = connect_to_url(&stereo_listen_url(proxy, DUAL_QUERY)).await;
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
    let mic_recording = partial_then_close_recording("Mic partial", 0, 300, "mic done");
    let spk_recording = partial_then_close_recording("Speaker partial", 0, 300, "speaker done");

    let soniox_mock = start_split_mock_ws([mic_recording, spk_recording]).await;
    let deepgram_mock = start_mock_ws().await;
    let proxy = start_proxy(Some(&deepgram_mock.ws_url()), Some(&soniox_mock.ws_url())).await;

    let mut ws = connect_to_url(&stereo_listen_url(proxy, DUAL_QUERY)).await;
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

    let client_messages = common::wait_for(TIMEOUT, || {
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

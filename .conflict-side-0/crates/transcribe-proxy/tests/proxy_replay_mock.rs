mod common;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use common::{
    CloseInfo, MessageKind, MockUpstreamConfig, collect_text_messages, connect_to_proxy,
    load_fixture, start_mock_server_with_config, start_server_with_upstream_url,
};
use owhisper_client::Provider;

const TEST_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

struct ReplayResult {
    messages: Vec<String>,
    close_info: CloseInfo,
}

async fn run_replay_case(
    fixture_name: &str,
    provider: Provider,
    model: &str,
    config: MockUpstreamConfig,
) -> ReplayResult {
    let recording = load_fixture(fixture_name);
    let mock_handle = start_mock_server_with_config(recording, config)
        .await
        .expect("failed to start mock server");
    let proxy_addr = start_server_with_upstream_url(provider, &mock_handle.ws_url()).await;
    let ws_stream = connect_to_proxy(proxy_addr, provider, model).await;
    let (messages, close_info) = collect_text_messages(ws_stream, TEST_RESPONSE_TIMEOUT).await;

    ReplayResult {
        messages,
        close_info,
    }
}

fn assert_replay_messages(result: &ReplayResult, expected_fragments: &[&str], context: &str) {
    assert!(
        !result.messages.is_empty(),
        "{context}: expected to receive messages"
    );
    for expected in expected_fragments {
        assert!(
            result
                .messages
                .iter()
                .any(|message| message.contains(expected)),
            "{context}: {:?}",
            result.messages
        );
    }
}

fn assert_any_message_contains(messages: &[String], needles: &[&str], context: &str) {
    assert!(
        messages
            .iter()
            .any(|message| needles.iter().any(|needle| message.contains(needle))),
        "{context}: {messages:?}"
    );
}

fn assert_close_code(close_info: CloseInfo, expected: u16, context: &str) {
    if let Some((code, _reason)) = close_info {
        assert_eq!(code, expected, "{context}");
    }
}

fn assert_close_code_in(close_info: CloseInfo, expected: &[u16], context: &str) {
    if let Some((code, _reason)) = close_info {
        assert!(
            expected.contains(&code),
            "{context}, got {code}, expected one of {expected:?}"
        );
    }
}

#[tokio::test]
async fn test_deepgram_normal_transcription_replay() {
    let _ = tracing_subscriber::fmt::try_init();

    let result = run_replay_case(
        "deepgram_normal.jsonl",
        Provider::Deepgram,
        "nova-3",
        MockUpstreamConfig::default(),
    )
    .await;

    assert_replay_messages(
        &result,
        &["Hello world", "This is a test"],
        "expected deepgram transcripts",
    );
    assert_close_code(result.close_info, 1000, "expected normal close code 1000");
}

#[tokio::test]
async fn test_deepgram_auth_error_replay() {
    let _ = tracing_subscriber::fmt::try_init();

    let result = run_replay_case(
        "deepgram_auth_error.jsonl",
        Provider::Deepgram,
        "nova-3",
        MockUpstreamConfig::default(),
    )
    .await;

    assert!(
        !result.messages.is_empty(),
        "expected to receive error message"
    );
    assert_any_message_contains(
        &result.messages,
        &["INVALID_AUTH", "Invalid credentials"],
        "expected auth error message",
    );
    assert_close_code_in(
        result.close_info,
        &[4401, 1008],
        "expected close code 4401 or 1008",
    );
}

#[tokio::test]
async fn test_deepgram_rate_limit_replay() {
    let _ = tracing_subscriber::fmt::try_init();

    let result = run_replay_case(
        "deepgram_rate_limit.jsonl",
        Provider::Deepgram,
        "nova-3",
        MockUpstreamConfig::default(),
    )
    .await;

    assert_any_message_contains(
        &result.messages,
        &["TOO_MANY_REQUESTS", "Too many requests"],
        "expected rate limit error message",
    );
    assert_close_code_in(
        result.close_info,
        &[4429, 1008],
        "expected close code 4429 or 1008",
    );
}

#[tokio::test]
async fn test_soniox_normal_transcription_replay() {
    let _ = tracing_subscriber::fmt::try_init();

    let result = run_replay_case(
        "soniox_normal.jsonl",
        Provider::Soniox,
        "stt-v3",
        MockUpstreamConfig::default(),
    )
    .await;

    assert_replay_messages(
        &result,
        &["Hello world", "Soniox"],
        "expected soniox transcripts",
    );
    assert_close_code(result.close_info, 1000, "expected normal close code 1000");
}

#[tokio::test]
async fn test_soniox_error_replay() {
    let _ = tracing_subscriber::fmt::try_init();

    let result = run_replay_case(
        "soniox_error.jsonl",
        Provider::Soniox,
        "stt-v3",
        MockUpstreamConfig::default(),
    )
    .await;

    assert_any_message_contains(
        &result.messages,
        &["error_code", "Cannot continue request"],
        "expected error message",
    );
    assert_close_code_in(
        result.close_info,
        &[4500, 1011],
        "expected close code 4500 or 1011",
    );
}

#[tokio::test]
async fn test_proxy_forwards_all_messages() {
    let _ = tracing_subscriber::fmt::try_init();

    let recording = load_fixture("deepgram_normal.jsonl");
    let expected_text_count = recording
        .server_messages()
        .filter(|m| matches!(m.kind, MessageKind::Text))
        .count();

    let result = run_replay_case(
        "deepgram_normal.jsonl",
        Provider::Deepgram,
        "nova-3",
        MockUpstreamConfig::default(),
    )
    .await;

    assert_eq!(
        result.messages.len(),
        expected_text_count,
        "Expected {} messages, got {}",
        expected_text_count,
        result.messages.len()
    );
}

#[tokio::test]
async fn test_proxy_handles_client_disconnect() {
    let _ = tracing_subscriber::fmt::try_init();

    let recording = load_fixture("deepgram_normal.jsonl");
    let mock_handle = start_mock_server_with_config(
        recording,
        MockUpstreamConfig::default()
            .use_timing(true)
            .max_delay_ms(100),
    )
    .await
    .expect("Failed to start mock server");

    let proxy_addr =
        start_server_with_upstream_url(Provider::Deepgram, &mock_handle.ws_url()).await;

    let ws_stream = connect_to_proxy(proxy_addr, Provider::Deepgram, "nova-3").await;
    let (mut sender, mut receiver) = ws_stream.split();

    if let Some(msg) = receiver.next().await {
        assert!(msg.is_ok(), "Expected first message to succeed");
    }

    let _ = sender.send(Message::Close(None)).await;
}

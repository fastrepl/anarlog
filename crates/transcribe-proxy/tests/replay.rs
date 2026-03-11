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

    assert!(!result.messages.is_empty(), "expected to receive messages");

    let has_hello_world = result
        .messages
        .iter()
        .any(|message| message.contains("Hello world"));
    let has_test = result
        .messages
        .iter()
        .any(|message| message.contains("This is a test"));
    assert!(has_hello_world, "Expected 'Hello world' transcript");
    assert!(has_test, "Expected 'This is a test' transcript");

    if let Some((code, _reason)) = result.close_info {
        assert_eq!(code, 1000, "Expected normal close code 1000");
    }
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
    let has_auth_error = result
        .messages
        .iter()
        .any(|message| message.contains("INVALID_AUTH") || message.contains("Invalid credentials"));
    assert!(has_auth_error, "Expected auth error message");

    if let Some((code, _reason)) = result.close_info {
        assert!(
            code == 4401 || code == 1008,
            "Expected close code 4401 or 1008, got {}",
            code
        );
    }
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

    let has_rate_limit = result.messages.iter().any(|message| {
        message.contains("TOO_MANY_REQUESTS") || message.contains("Too many requests")
    });
    assert!(has_rate_limit, "Expected rate limit error message");

    if let Some((code, _reason)) = result.close_info {
        assert!(
            code == 4429 || code == 1008,
            "Expected close code 4429 or 1008, got {}",
            code
        );
    }
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

    assert!(!result.messages.is_empty(), "expected to receive messages");

    let has_hello_world = result
        .messages
        .iter()
        .any(|message| message.contains("Hello world"));
    let has_soniox = result
        .messages
        .iter()
        .any(|message| message.contains("Soniox"));
    assert!(has_hello_world, "Expected 'Hello world' transcript");
    assert!(has_soniox, "Expected 'Soniox' transcript");

    if let Some((code, _reason)) = result.close_info {
        assert_eq!(code, 1000, "Expected normal close code 1000");
    }
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

    let has_error = result.messages.iter().any(|message| {
        message.contains("error_code") || message.contains("Cannot continue request")
    });
    assert!(has_error, "Expected error message");

    if let Some((code, _reason)) = result.close_info {
        assert!(
            code == 4500 || code == 1011,
            "Expected close code 4500 or 1011, got {}",
            code
        );
    }
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

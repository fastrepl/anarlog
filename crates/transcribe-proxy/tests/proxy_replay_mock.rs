mod common;

use std::time::Duration;

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
async fn replays_normal_transcriptions() {
    let _ = tracing_subscriber::fmt::try_init();

    let cases = [
        (
            "deepgram_normal.jsonl",
            Provider::Deepgram,
            "nova-3",
            &["Hello world", "This is a test"][..],
        ),
        (
            "soniox_normal.jsonl",
            Provider::Soniox,
            "stt-v3",
            &["Hello world", "Soniox"][..],
        ),
    ];

    for (fixture, provider, model, fragments) in cases {
        let expected_text_count = (provider == Provider::Deepgram).then(|| {
            load_fixture(fixture)
                .server_messages()
                .filter(|message| matches!(message.kind, MessageKind::Text))
                .count()
        });
        let result = run_replay_case(fixture, provider, model, MockUpstreamConfig::default()).await;

        assert_replay_messages(&result, fragments, fixture);
        assert_close_code(result.close_info, 1000, "expected normal close code 1000");
        if let Some(expected_text_count) = expected_text_count {
            assert_eq!(
                result.messages.len(),
                expected_text_count,
                "Expected {} messages, got {}",
                expected_text_count,
                result.messages.len()
            );
        }
    }
}

#[tokio::test]
async fn replays_provider_errors() {
    let _ = tracing_subscriber::fmt::try_init();

    let cases = [
        (
            "deepgram_auth_error.jsonl",
            Provider::Deepgram,
            "nova-3",
            &[4401, 1008][..],
            &["INVALID_AUTH", "Invalid credentials"][..],
        ),
        (
            "deepgram_rate_limit.jsonl",
            Provider::Deepgram,
            "nova-3",
            &[4429, 1008][..],
            &["TOO_MANY_REQUESTS", "Too many requests"][..],
        ),
        (
            "soniox_error.jsonl",
            Provider::Soniox,
            "stt-v3",
            &[4500, 1011][..],
            &["error_code", "Cannot continue request"][..],
        ),
    ];

    for (fixture, provider, model, close_codes, needles) in cases {
        let result = run_replay_case(fixture, provider, model, MockUpstreamConfig::default()).await;

        assert_any_message_contains(&result.messages, needles, fixture);
        assert_close_code_in(result.close_info, close_codes, fixture);
    }
}

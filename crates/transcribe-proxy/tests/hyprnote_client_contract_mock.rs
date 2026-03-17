mod common;

use std::time::Duration;

use common::{
    ClientStreamResult, MockUpstreamConfig, batch_upstream_url, close_only_recording,
    collect_streaming_via_client, collect_streaming_via_client_result, english, load_fixture,
    sample_response, send_batch_via_deepgram_client, send_batch_via_hyprnote_client,
    send_streaming_via_client, single_response_recording, start_mock_batch_upstream,
    start_mock_server_with_config, start_mock_ws, start_proxy, start_proxy_under_stt,
    wait_for_first_batch_query, wait_for_first_request,
};
use hypr_language::ISO639;
use owhisper_client::Provider;
use owhisper_interface::batch::Response;
use owhisper_interface::stream::StreamResponse;

const TIMEOUT: Duration = Duration::from_secs(2);

fn batch_transcript(response: &Response) -> &str {
    response
        .results
        .channels
        .first()
        .and_then(|channel| channel.alternatives.first())
        .map(|alt| alt.transcript.as_str())
        .unwrap_or("")
}

fn assert_terminal_error_contains(
    result: &ClientStreamResult,
    expected_fragments: &[&str],
    context: &str,
) {
    let error = result
        .terminal_error
        .as_deref()
        .unwrap_or_else(|| panic!("{context}: expected terminal client error"));

    assert!(
        expected_fragments
            .iter()
            .any(|fragment| error.contains(fragment)),
        "{context}: {error}"
    );
}

fn sample_error_response(error_code: i32, error_message: &str, provider: &str) -> StreamResponse {
    StreamResponse::ErrorResponse {
        error_code: Some(error_code),
        error_message: error_message.to_string(),
        provider: provider.to_string(),
    }
}

#[tokio::test]
async fn streaming_client_adapter_resolves_cloud_model() {
    let mock = start_mock_ws().await;
    let proxy = start_proxy(Some(&mock.ws_url()), None).await;

    send_streaming_via_client(proxy, "cloud", english()).await;
    let req = wait_for_first_request(&mock, TIMEOUT).await;

    assert!(
        req.contains("model=nova-3"),
        "should resolve cloud -> nova-3 for en: {req}"
    );
    assert!(
        !req.contains("model=cloud"),
        "meta model should not leak upstream: {req}"
    );
    assert!(
        req.contains("sample_rate=16000"),
        "listen params should reach upstream: {req}"
    );
    assert!(
        req.contains("channels=1"),
        "listen params should reach upstream: {req}"
    );
}

#[tokio::test]
async fn streaming_hyprnote_client_accepts_single_proxy_response_object() {
    let upstream = start_mock_server_with_config(
        single_response_recording(&sample_response("hello from proxy")),
        MockUpstreamConfig::default(),
    )
    .await
    .expect("failed to start mock ws server");
    let proxy = start_proxy(Some(&upstream.ws_url()), None).await;

    let responses = collect_streaming_via_client(proxy, "cloud", english(), TIMEOUT).await;

    assert_eq!(
        responses.len(),
        1,
        "single proxy response objects should produce one client event"
    );

    match &responses[0] {
        StreamResponse::TranscriptResponse { channel, .. } => {
            assert_eq!(channel.alternatives[0].transcript, "hello from proxy");
        }
        other => panic!("expected transcript response, got {other:?}"),
    }
}

#[tokio::test]
async fn streaming_hyprnote_client_accepts_single_proxy_error_response_object() {
    let upstream = start_mock_server_with_config(
        single_response_recording(&sample_error_response(
            4401,
            "Invalid credentials.",
            "deepgram",
        )),
        MockUpstreamConfig::default(),
    )
    .await
    .expect("failed to start mock ws server");
    let proxy = start_proxy(Some(&upstream.ws_url()), None).await;

    let result = collect_streaming_via_client_result(proxy, "cloud", english(), TIMEOUT).await;

    assert_eq!(
        result.responses.len(),
        1,
        "single proxy error objects should produce one client event"
    );
    assert!(
        result.terminal_error.is_none(),
        "mock close should not leak through once the client received an error response: {:?}",
        result.terminal_error
    );

    match &result.responses[0] {
        StreamResponse::ErrorResponse {
            error_code,
            error_message,
            provider,
        } => {
            assert_eq!(*error_code, Some(4401));
            assert_eq!(error_message, "Invalid credentials.");
            assert_eq!(provider, "deepgram");
        }
        other => panic!("expected error response, got {other:?}"),
    }
}

#[tokio::test]
async fn streaming_hyprnote_client_accepts_single_proxy_rate_limit_error_object() {
    let upstream = start_mock_server_with_config(
        single_response_recording(&sample_error_response(
            4429,
            "Too many requests. Please try again later",
            "deepgram",
        )),
        MockUpstreamConfig::default(),
    )
    .await
    .expect("failed to start mock ws server");
    let proxy = start_proxy(Some(&upstream.ws_url()), None).await;

    let result = collect_streaming_via_client_result(proxy, "cloud", english(), TIMEOUT).await;

    assert_eq!(
        result.responses.len(),
        1,
        "single proxy error objects should produce one client event"
    );
    assert!(
        result.terminal_error.is_none(),
        "mock close should not leak through once the client received an error response: {:?}",
        result.terminal_error
    );

    match &result.responses[0] {
        StreamResponse::ErrorResponse {
            error_code,
            error_message,
            provider,
        } => {
            assert_eq!(*error_code, Some(4429));
            assert_eq!(error_message, "Too many requests. Please try again later");
            assert_eq!(provider, "deepgram");
        }
        other => panic!("expected error response, got {other:?}"),
    }
}

#[tokio::test]
async fn streaming_hyprnote_client_surfaces_soniox_provider_failure() {
    let upstream = start_mock_server_with_config(
        load_fixture("soniox_error.jsonl"),
        MockUpstreamConfig::default(),
    )
    .await
    .expect("failed to start mock ws server");
    let proxy = start_proxy(None, Some(&upstream.ws_url())).await;

    let result = collect_streaming_via_client_result(
        proxy,
        "cloud",
        vec![ISO639::En.into(), ISO639::Ko.into()],
        TIMEOUT,
    )
    .await;

    assert_eq!(
        result.responses.len(),
        1,
        "provider failures should emit a single client error response: {:?}",
        result.responses
    );
    assert!(
        result.terminal_error.is_none(),
        "mock close should not leak through once the client received an error response: {:?}",
        result.terminal_error
    );

    match &result.responses[0] {
        StreamResponse::ErrorResponse {
            error_code,
            error_message,
            provider,
        } => {
            assert_eq!(*error_code, Some(503));
            assert!(
                error_message.contains("Cannot continue request"),
                "unexpected provider error message: {error_message}"
            );
            assert_eq!(provider, "soniox");
        }
        other => panic!("expected error response, got {other:?}"),
    }
}

#[tokio::test]
async fn streaming_hyprnote_client_surfaces_abnormal_close_without_response() {
    let upstream = start_mock_server_with_config(
        close_only_recording(0, 1011, "upstream_failed"),
        MockUpstreamConfig::default(),
    )
    .await
    .expect("failed to start mock ws server");
    let proxy = start_proxy(Some(&upstream.ws_url()), None).await;

    let result = collect_streaming_via_client_result(proxy, "cloud", english(), TIMEOUT).await;

    assert!(
        result.responses.is_empty(),
        "abnormal closes without payloads should not emit client responses: {:?}",
        result.responses
    );
    assert_terminal_error_contains(
        &result,
        &["ResetWithoutClosingHandshake", "Protocol"],
        "abnormal closes without responses should surface as client stream errors",
    );
}

#[tokio::test]
async fn batch_client_hyprnote_adapter_uses_proxy_sync_path_under_stt() {
    let batch = start_mock_batch_upstream().await;
    let upstream_url = batch_upstream_url(batch.addr);
    let proxy = start_proxy_under_stt(Provider::Deepgram, Some(&upstream_url), None).await;

    let response = send_batch_via_hyprnote_client(proxy, "cloud", english()).await;
    let query = wait_for_first_batch_query(&batch, TIMEOUT).await;

    assert_eq!(
        batch_transcript(&response),
        "ok",
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

    let response = send_batch_via_deepgram_client(proxy, "nova-2", english()).await;
    let query = wait_for_first_batch_query(&batch, TIMEOUT).await;

    assert_eq!(
        batch_transcript(&response),
        "ok",
        "passthrough batch should return upstream response"
    );
    assert!(
        query.contains("model=nova-2"),
        "passthrough batch should preserve the direct-provider request shape: {query}"
    );
}

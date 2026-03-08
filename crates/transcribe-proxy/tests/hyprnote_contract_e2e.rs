mod common;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{Json, Router, extract::RawQuery, response::IntoResponse, routing::post};
use bytes::Bytes;
use common::{
    Direction, MockUpstreamConfig, WsMessage, WsRecording, start_mock_server_with_config,
};
use owhisper_client::{BatchClient, DeepgramAdapter, HyprnoteAdapter, ListenClient, Provider};
use owhisper_interface::{ControlMessage, ListenParams, MixedMessage};
use tokio_tungstenite::connect_async;
use transcribe_proxy::{HyprnoteRoutingConfig, SttProxyConfig};

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

async fn start_proxy(deepgram_upstream: Option<&str>, soniox_upstream: Option<&str>) -> SocketAddr {
    start_proxy_with(
        Provider::Deepgram,
        false,
        deepgram_upstream,
        soniox_upstream,
    )
    .await
}

async fn start_proxy_under_stt(
    default_provider: Provider,
    deepgram_upstream: Option<&str>,
    soniox_upstream: Option<&str>,
) -> SocketAddr {
    start_proxy_with(default_provider, true, deepgram_upstream, soniox_upstream).await
}

async fn start_proxy_with(
    default_provider: Provider,
    mount_under_stt: bool,
    deepgram_upstream: Option<&str>,
    soniox_upstream: Option<&str>,
) -> SocketAddr {
    let mut env = transcribe_proxy::Env::default();
    if deepgram_upstream.is_some() {
        env.stt.deepgram_api_key = Some("test-key".to_string());
    }
    if soniox_upstream.is_some() {
        env.stt.soniox_api_key = Some("test-key".to_string());
    }

    let supabase_env = hypr_api_env::SupabaseEnv {
        supabase_url: String::new(),
        supabase_anon_key: String::new(),
        supabase_service_role_key: String::new(),
    };

    let mut config = SttProxyConfig::new(&env, &supabase_env)
        .with_default_provider(default_provider)
        .with_hyprnote_routing(HyprnoteRoutingConfig::default());

    if let Some(url) = deepgram_upstream {
        config = config.with_upstream_url(Provider::Deepgram, url);
    }
    if let Some(url) = soniox_upstream {
        config = config.with_upstream_url(Provider::Soniox, url);
    }

    let app = if mount_under_stt {
        Router::new().nest("/stt", transcribe_proxy::router(config))
    } else {
        transcribe_proxy::router(config)
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    addr
}

async fn poll_first<T>(mut f: impl FnMut() -> Option<T>, timeout: Duration) -> T {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out within {timeout:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn send_streaming(addr: SocketAddr, query: &str) {
    let url = format!(
        "ws://{addr}/listen?provider=hyprnote&encoding=linear16&sample_rate=16000&channels=1&{query}"
    );
    let (mut ws, _) = connect_async(&url).await.expect("failed to connect");
    let _ = ws.close(None).await;
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

struct MockBatchUpstream {
    addr: SocketAddr,
    queries: Arc<Mutex<Vec<String>>>,
}

async fn start_mock_batch_upstream() -> MockBatchUpstream {
    let queries: Arc<Mutex<Vec<String>>> = Default::default();
    let captured = queries.clone();

    let app = Router::new().route(
        "/v1/listen",
        post(move |query: RawQuery| {
            let captured = captured.clone();
            async move {
                if let Ok(mut v) = captured.lock() {
                    v.push(query.0.unwrap_or_default());
                }
                Json(serde_json::json!({
                  "metadata": {},
                  "results": {
                    "channels": [{
                      "alternatives": [{
                        "transcript": "ok",
                        "confidence": 1.0,
                        "words": []
                      }]
                    }]
                  }
                }))
                .into_response()
            }
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    MockBatchUpstream { addr, queries }
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

#[tokio::test]
async fn streaming_cloud_model_resolved_for_deepgram() {
    let mock = start_mock_ws().await;
    let proxy = start_proxy(Some(&mock.ws_url()), None).await;

    send_streaming(proxy, "model=cloud&language=en").await;
    let req = poll_first(|| mock.captured_requests().first().cloned(), TIMEOUT).await;

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
    let req = poll_first(|| mock.captured_requests().first().cloned(), TIMEOUT).await;

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
    let sox_req = poll_first(|| sox_mock.captured_requests().first().cloned(), TIMEOUT).await;

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
    let req = poll_first(|| mock.captured_requests().first().cloned(), TIMEOUT).await;

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
    let req = poll_first(|| mock.captured_requests().first().cloned(), TIMEOUT).await;

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
    let proxy = start_proxy(Some(&format!("http://{}/v1", batch.addr)), None).await;

    send_batch(proxy, "model=cloud&language=en").await;
    let query = poll_first(|| batch.queries.lock().ok()?.first().cloned(), TIMEOUT).await;

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
    let proxy = start_proxy(Some(&format!("http://{}/v1", batch.addr)), None).await;

    send_batch(proxy, "model=nova-3&language=en").await;
    let query = poll_first(|| batch.queries.lock().ok()?.first().cloned(), TIMEOUT).await;

    assert!(
        query.contains("model=nova-3"),
        "explicit model should be preserved: {query}"
    );
}

#[tokio::test]
async fn batch_client_hyprnote_adapter_uses_proxy_sync_path_under_stt() {
    let batch = start_mock_batch_upstream().await;
    let proxy = start_proxy_under_stt(
        Provider::Deepgram,
        Some(&format!("http://{}/v1", batch.addr)),
        None,
    )
    .await;

    let response =
        send_batch_via_hyprnote_client(proxy, "cloud", vec![hypr_language::ISO639::En.into()])
            .await;
    let query = poll_first(|| batch.queries.lock().ok()?.first().cloned(), TIMEOUT).await;

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
    let proxy = start_proxy_under_stt(
        Provider::Soniox,
        Some(&format!("http://{}/v1", batch.addr)),
        None,
    )
    .await;

    let response =
        send_batch_via_deepgram_client(proxy, "nova-2", vec![hypr_language::ISO639::En.into()])
            .await;
    let query = poll_first(|| batch.queries.lock().ok()?.first().cloned(), TIMEOUT).await;

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

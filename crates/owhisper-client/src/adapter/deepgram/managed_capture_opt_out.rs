//! Request-level contracts for the managed Nova-3 live and recovery paths.
//! All requests go to an owned loopback fixture, with fictional bytes and keys.

use anlg_ws_client::client::WebSocketConnectPolicy;
use owhisper_interface::ListenParams;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use crate::{BatchSttAdapter, DeepgramAdapter, ListenClient};

fn capture_params() -> ListenParams {
    ListenParams {
        model: Some("nova-3".into()),
        channels: 2,
        languages: vec![anlg_language::ISO639::En.into()],
        ..Default::default()
    }
}

fn assert_opted_out(request: &wiremock::Request) {
    let flags: Vec<_> = request
        .url
        .query_pairs()
        .filter(|(key, _)| key == "mip_opt_out")
        .map(|(_, value)| value.into_owned())
        .collect();
    assert_eq!(
        flags,
        ["true"],
        "managed capture must send one unambiguous opt-out"
    );
    assert!(
        request
            .url
            .query_pairs()
            .any(|(key, value)| key == "model" && value == "nova-3")
    );
    assert!(
        request
            .url
            .query_pairs()
            .any(|(key, value)| key == "multichannel" && value == "true")
    );
    assert_eq!(
        request.headers.get("authorization").unwrap(),
        "Token fictional-managed-capture-key"
    );
}

#[tokio::test]
async fn managed_capture_opt_out_reaches_each_live_handshake_and_connection_retry() {
    let server = MockServer::start().await;
    // Refuse the upgrade: no audio is sent. A transient response makes the real
    // transport retry its request, so both observed attempts must opt out.
    Mock::given(method("GET"))
        .and(path("/v1/listen"))
        .respond_with(ResponseTemplate::new(503))
        .expect(2)
        .mount(&server)
        .await;
    let client = ListenClient::builder()
        .api_base(format!("{}/v1", server.uri()))
        .api_key("fictional-managed-capture-key")
        .params(capture_params())
        .connect_policy(WebSocketConnectPolicy {
            connect_timeout: std::time::Duration::from_secs(2),
            max_attempts: 2,
            retry_delay: std::time::Duration::from_millis(1),
        })
        .build_with_channels(2)
        .await
        .unwrap();
    let result = client
        .from_realtime_audio(futures_util::stream::empty())
        .await;
    assert!(result.is_err());
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        assert_opted_out(request);
        assert!(request.body.is_empty());
        assert_eq!(request.headers.get("upgrade").unwrap(), "websocket");
    }
}

#[tokio::test]
async fn managed_capture_opt_out_reaches_the_actual_stereo_recovery_batch_post() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/listen"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "metadata": {},
            "results": {"channels": [
                {"alternatives": [{"transcript": "Fictional microphone decision.", "confidence": 1.0, "words": []}]},
                {"alternatives": [{"transcript": "Fictional call-side decision.", "confidence": 1.0, "words": []}]}
            ]}
        })))
        .expect(1)
        .mount(&server).await;
    let directory = tempfile::tempdir().unwrap();
    let audio_path = directory.path().join("fictional.wav");
    let audio = b"Fictional stereo fixture bytes; never sent to a provider";
    std::fs::write(&audio_path, audio).unwrap();
    let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let params = capture_params();
    let result = DeepgramAdapter
        .transcribe_file(
            &client,
            &format!("{}/v1", server.uri()),
            "fictional-managed-capture-key",
            &params,
            &audio_path,
        )
        .await
        .unwrap();
    assert_eq!(result.results.channels.len(), 2);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_opted_out(&requests[0]);
    assert_eq!(requests[0].body, audio);
    assert_eq!(
        requests[0].headers.get("content-type").unwrap(),
        "audio/wav"
    );
}

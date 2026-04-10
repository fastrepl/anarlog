mod common;

use std::time::Duration;

use common::{
    batch_upstream_url, send_batch, send_streaming, start_mock_batch_upstream, start_mock_ws,
    start_proxy, wait_for_first_batch_query, wait_for_first_request,
};

const TIMEOUT: Duration = Duration::from_secs(2);

#[tokio::test]
async fn streaming_cloud_model_resolved_for_deepgram() {
    let mock = start_mock_ws().await;
    let proxy = start_proxy(Some(&mock.ws_url()), None).await;

    send_streaming(proxy, "model=cloud&language=en").await;
    let req = wait_for_first_request(&mock, TIMEOUT).await;

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
    let req = wait_for_first_request(&mock, TIMEOUT).await;

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
    let sox_req = wait_for_first_request(&sox_mock, TIMEOUT).await;

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
    let req = wait_for_first_request(&mock, TIMEOUT).await;

    assert!(
        req.contains("model=nova-3"),
        "explicit model should be preserved: {req}"
    );
}

#[tokio::test]
async fn batch_cloud_model_resolved_for_deepgram() {
    let batch = start_mock_batch_upstream().await;
    let upstream_url = batch_upstream_url(batch.addr);
    let proxy = start_proxy(Some(&upstream_url), None).await;

    send_batch(proxy, "model=cloud&language=en").await;
    let query = wait_for_first_batch_query(&batch, TIMEOUT).await;

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
    let query = wait_for_first_batch_query(&batch, TIMEOUT).await;

    assert!(
        query.contains("model=nova-3"),
        "explicit model should be preserved: {query}"
    );
}

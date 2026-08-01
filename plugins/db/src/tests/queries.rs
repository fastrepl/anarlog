use anlg_db_reactive::QueryEventSink;
use serde_json::json;

use crate::{QueryEvent, runtime};

use super::support::{capture_channel, next_event, setup_runtime};

#[tokio::test]
async fn query_event_channel_sends_result_payload() {
    let (channel, events) = capture_channel();
    let sink = runtime::QueryEventChannel::new(channel);

    sink.send_result(vec![json!({ "id": "note-1" })]).unwrap();

    let event = next_event(&events, 0).await.unwrap();
    assert_eq!(event, QueryEvent::Result(vec![json!({ "id": "note-1" })]));
}

#[tokio::test]
async fn query_event_channel_sends_error_payload() {
    let (channel, events) = capture_channel();
    let sink = runtime::QueryEventChannel::new(channel);

    sink.send_error("boom".to_string()).unwrap();

    let event = next_event(&events, 0).await.unwrap();
    assert_eq!(event, QueryEvent::Error("boom".to_string()));
}

#[test]
fn query_event_serializes_with_tagged_shape() {
    let result = serde_json::to_value(QueryEvent::Result(vec![json!({ "id": "note-1" })])).unwrap();
    let error = serde_json::to_value(QueryEvent::Error("boom".to_string())).unwrap();

    assert_eq!(
        result,
        json!({ "event": "result", "data": [{ "id": "note-1" }] })
    );
    assert_eq!(error, json!({ "event": "error", "data": "boom" }));
}

#[tokio::test]
async fn subscribe_sends_initial_result_through_channel() {
    let (_dir, runtime) = setup_runtime().await;
    let (channel, events) = capture_channel();

    runtime
        .subscribe(
            "SELECT id, title FROM templates WHERE id = 'missing-template' ORDER BY id".to_string(),
            vec![],
            runtime::QueryEventChannel::new(channel),
        )
        .await
        .unwrap();

    let event = next_event(&events, 0).await.unwrap();
    assert_eq!(event, QueryEvent::Result(Vec::new()));
}

#[tokio::test]
async fn invalid_sql_sends_error_through_channel() {
    let (_dir, runtime) = setup_runtime().await;
    let (channel, events) = capture_channel();

    runtime
        .subscribe(
            "SELECT * FROM missing_table".to_string(),
            vec![],
            runtime::QueryEventChannel::new(channel),
        )
        .await
        .unwrap();

    let event = next_event(&events, 0).await.unwrap();
    assert!(matches!(event, QueryEvent::Error(_)));
}

mod common;

use common::{
    TestSink, expect_empty_result, expect_error, expect_no_event, expect_result, insert_daily_note,
    subscribe, subscribe_all_daily_notes, wait_until_subscription_removed,
};
use db_reactive::Error;
use serde_json::json;

#[tokio::test]
async fn invalid_sql_sends_error_event() {
    let (_dir, _pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    subscribe(&runtime, "SELECT * FROM missing_table", Vec::new(), sink)
        .await
        .unwrap();

    let _error = expect_error(&events, 0).await;
}

#[tokio::test]
async fn initial_sink_failure_rolls_back_registration() {
    let (_dir, _pool, runtime) = common::setup_runtime().await;
    let (sink, _events) = TestSink::fail_after(0);

    let error = subscribe_all_daily_notes(&runtime, sink)
        .await
        .err()
        .expect("subscription should fail when the sink rejects the initial result");

    assert!(matches!(error, Error::Sink(message) if message == "sink closed"));
}

#[tokio::test]
async fn stale_subscribers_are_removed_after_send_failures() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::fail_after(1);

    let registration = subscribe_all_daily_notes(&runtime, sink).await.unwrap();

    expect_empty_result(&events, 0).await;

    insert_daily_note(&pool, "note-stale", "2026-04-15", "user-1").await;

    expect_no_event(&events, 1).await;
    wait_until_subscription_removed(&runtime, &registration.id).await;
}

#[tokio::test]
async fn failing_subscriber_does_not_break_other_reactive_subscribers() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (failing_sink, failing_events) = TestSink::fail_after(1);
    let (healthy_sink, healthy_events) = TestSink::capture();

    let failing_registration = subscribe_all_daily_notes(&runtime, failing_sink)
        .await
        .unwrap();
    let healthy_registration = subscribe_all_daily_notes(&runtime, healthy_sink)
        .await
        .unwrap();

    expect_empty_result(&failing_events, 0).await;
    expect_empty_result(&healthy_events, 0).await;

    insert_daily_note(&pool, "note-shared-1", "2026-04-16", "user-1").await;

    expect_no_event(&failing_events, 1).await;
    expect_result(
        &healthy_events,
        1,
        vec![json!({ "id": "note-shared-1", "date": "2026-04-16" })],
    )
    .await;

    wait_until_subscription_removed(&runtime, &failing_registration.id).await;

    assert!(
        runtime
            .dependency_analysis(&healthy_registration.id)
            .await
            .is_some()
    );

    insert_daily_note(&pool, "note-shared-2", "2026-04-17", "user-2").await;

    expect_result(
        &healthy_events,
        2,
        vec![
            json!({ "id": "note-shared-1", "date": "2026-04-16" }),
            json!({ "id": "note-shared-2", "date": "2026-04-17" }),
        ],
    )
    .await;
}

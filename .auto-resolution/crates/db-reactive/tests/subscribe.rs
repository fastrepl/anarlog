mod common;

use std::time::Duration;

use common::{
    TestEvent, TestSink, expect_empty_result, expect_no_event, expect_result, insert_daily_note,
    insert_daily_summary, next_result_rows, subscribe_all_daily_notes,
    subscribe_all_daily_summaries, subscribe_daily_note_by_id, wait_for_stable_event_count,
};
use serde_json::json;

#[tokio::test]
async fn sends_initial_result() {
    let (_dir, _pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    subscribe_all_daily_notes(&runtime, sink).await.unwrap();

    expect_empty_result(&events, 0).await;
}

#[tokio::test]
async fn dependent_writes_trigger_refresh() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    subscribe_all_daily_notes(&runtime, sink).await.unwrap();
    expect_empty_result(&events, 0).await;

    insert_daily_note(&pool, "note-1", "2026-04-13", "user-1").await;

    expect_result(
        &events,
        1,
        vec![json!({ "id": "note-1", "date": "2026-04-13" })],
    )
    .await;
}

#[tokio::test]
async fn updates_to_dependent_rows_trigger_refresh() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    insert_daily_note(&pool, "note-update", "2026-04-13", "user-1").await;

    subscribe_daily_note_by_id(&runtime, "note-update", sink)
        .await
        .unwrap();

    expect_result(
        &events,
        0,
        vec![json!({ "id": "note-update", "date": "2026-04-13" })],
    )
    .await;

    sqlx::query("UPDATE daily_notes SET date = ? WHERE id = ?")
        .bind("2026-04-14")
        .bind("note-update")
        .execute(&pool)
        .await
        .unwrap();

    expect_result(
        &events,
        1,
        vec![json!({ "id": "note-update", "date": "2026-04-14" })],
    )
    .await;
}

#[tokio::test]
async fn deletes_from_dependent_rows_trigger_refresh() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    insert_daily_note(&pool, "note-delete", "2026-04-13", "user-1").await;

    subscribe_daily_note_by_id(&runtime, "note-delete", sink)
        .await
        .unwrap();

    expect_result(
        &events,
        0,
        vec![json!({ "id": "note-delete", "date": "2026-04-13" })],
    )
    .await;

    sqlx::query("DELETE FROM daily_notes WHERE id = ?")
        .bind("note-delete")
        .execute(&pool)
        .await
        .unwrap();

    expect_empty_result(&events, 1).await;
}

#[tokio::test]
async fn batched_writes_converge_on_latest_result() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    subscribe_all_daily_notes(&runtime, sink).await.unwrap();
    expect_empty_result(&events, 0).await;

    for idx in 1..=3 {
        insert_daily_note(
            &pool,
            &format!("note-batch-{idx}"),
            &format!("2026-04-1{idx}"),
            &format!("user-{idx}"),
        )
        .await;
    }

    let stable_count = wait_for_stable_event_count(&events, Duration::from_millis(100)).await;
    assert!(stable_count >= 2, "expected at least one refresh event");

    let final_event = events.lock().unwrap()[stable_count - 1].clone();
    assert_eq!(
        final_event,
        TestEvent::Result(vec![
            json!({ "id": "note-batch-1", "date": "2026-04-11" }),
            json!({ "id": "note-batch-2", "date": "2026-04-12" }),
            json!({ "id": "note-batch-3", "date": "2026-04-13" }),
        ])
    );
}

#[tokio::test]
async fn open_transactions_do_not_refresh_until_commit() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    subscribe_all_daily_notes(&runtime, sink).await.unwrap();
    expect_empty_result(&events, 0).await;

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
        .bind("note-in-tx")
        .bind("2026-04-16")
        .bind("{}")
        .bind("user-1")
        .execute(&mut *tx)
        .await
        .unwrap();

    expect_no_event(&events, 1).await;

    tx.commit().await.unwrap();

    let rows = next_result_rows(&events, 1).await;
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn rollback_after_write_does_not_refresh() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    subscribe_all_daily_notes(&runtime, sink).await.unwrap();
    expect_empty_result(&events, 0).await;

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
        .bind("note-rollback")
        .bind("2026-04-17")
        .bind("{}")
        .bind("user-1")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();

    expect_no_event(&events, 1).await;
}

#[tokio::test]
async fn unrelated_writes_do_not_trigger_refresh() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    insert_daily_note(&pool, "note-seed", "2026-04-12", "user-1").await;

    subscribe_all_daily_notes(&runtime, sink).await.unwrap();

    expect_result(
        &events,
        0,
        vec![json!({ "id": "note-seed", "date": "2026-04-12" })],
    )
    .await;

    insert_daily_summary(&pool, "summary-1", "note-seed", "2026-04-12").await;

    expect_no_event(&events, 1).await;
}

#[tokio::test]
async fn unsubscribe_stops_future_events() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    let registration = subscribe_all_daily_notes(&runtime, sink).await.unwrap();

    expect_empty_result(&events, 0).await;
    runtime.unsubscribe(&registration.id).await.unwrap();

    insert_daily_note(&pool, "note-2", "2026-04-14", "user-1").await;

    expect_no_event(&events, 1).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsubscribe_waits_for_in_flight_refresh_delivery() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events, send_block) = TestSink::with_blocked_send(1);

    let registration = subscribe_all_daily_notes(&runtime, sink).await.unwrap();

    expect_empty_result(&events, 0).await;
    insert_daily_note(&pool, "note-blocked-refresh", "2026-04-23", "user-1").await;

    send_block.wait_until_started().await;

    let unsubscribe = runtime.unsubscribe(&registration.id);
    tokio::pin!(unsubscribe);

    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut unsubscribe)
            .await
            .is_err()
    );

    send_block.release();
    unsubscribe.await.unwrap();

    expect_no_event(&events, 2).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unrelated_unsubscribe_is_not_blocked_by_another_subscriptions_delivery() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (blocked_sink, blocked_events, send_block) = TestSink::with_blocked_send(1);
    let (other_sink, other_events) = TestSink::capture();

    let blocked_registration = subscribe_all_daily_notes(&runtime, blocked_sink)
        .await
        .unwrap();
    let other_registration = subscribe_all_daily_summaries(&runtime, other_sink)
        .await
        .unwrap();

    expect_empty_result(&blocked_events, 0).await;
    expect_empty_result(&other_events, 0).await;

    insert_daily_note(&pool, "note-other-unsub", "2026-04-25", "user-1").await;

    send_block.wait_until_started().await;

    tokio::time::timeout(
        Duration::from_millis(50),
        runtime.unsubscribe(&other_registration.id),
    )
    .await
    .expect("unsubscribe should not wait for another subscription's blocked send")
    .unwrap();

    insert_daily_summary(
        &pool,
        "summary-after-unsub",
        "note-other-unsub",
        "2026-04-25",
    )
    .await;

    send_block.release();

    expect_result(
        &blocked_events,
        1,
        vec![json!({ "id": "note-other-unsub", "date": "2026-04-25" })],
    )
    .await;
    expect_no_event(&other_events, 1).await;

    runtime.unsubscribe(&blocked_registration.id).await.unwrap();
}

#[tokio::test]
async fn unsubscribe_returns_not_found_for_unknown_id() {
    let (_dir, _pool, runtime) = common::setup_runtime().await;

    let error = runtime.unsubscribe("missing").await.unwrap_err();
    assert!(matches!(error, db_reactive::Error::SubscriptionNotFound(id) if id == "missing"));
}

#[tokio::test]
async fn lagged_broadcast_receiver_resyncs_and_keeps_dispatcher_alive() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::with_delay(Duration::from_millis(5));

    subscribe_all_daily_notes(&runtime, sink).await.unwrap();
    expect_empty_result(&events, 0).await;

    for idx in 0..320 {
        insert_daily_note(
            &pool,
            &format!("note-lag-{idx}"),
            "2026-04-18",
            &format!("user-lag-{idx}"),
        )
        .await;
    }

    let _stable_count = wait_for_stable_event_count(&events, Duration::from_millis(100)).await;
    let before = events.lock().unwrap().len();

    insert_daily_note(&pool, "note-after-lag", "2026-04-19", "user-after-lag").await;

    let rows = next_result_rows(&events, before).await;
    assert!(rows.len() >= 321);
}

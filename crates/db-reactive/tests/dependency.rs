mod common;

use std::collections::HashSet;

use common::{
    TestSink, expect_empty_result, expect_error, expect_no_event, expect_result, insert_daily_note,
    subscribe,
};
use db_reactive::{DependencyAnalysis, DependencyTarget};
use serde_json::json;

#[tokio::test]
async fn view_subscriptions_refresh_after_base_table_writes() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    sqlx::query("CREATE TABLE view_notes (id TEXT PRIMARY KEY NOT NULL, body TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE VIEW view_notes_live AS SELECT id, body FROM view_notes")
        .execute(&pool)
        .await
        .unwrap();

    let registration = subscribe(
        &runtime,
        "SELECT id FROM view_notes_live WHERE body IS NOT NULL ORDER BY id",
        Vec::new(),
        sink,
    )
    .await
    .unwrap();

    assert_eq!(
        registration.analysis,
        DependencyAnalysis::Reactive {
            targets: HashSet::from([DependencyTarget::Table("view_notes".to_string())]),
        }
    );

    expect_empty_result(&events, 0).await;

    sqlx::query("INSERT INTO view_notes (id, body) VALUES (?, ?)")
        .bind("view-note-1")
        .bind("hello from view")
        .execute(&pool)
        .await
        .unwrap();

    expect_result(&events, 1, vec![json!({ "id": "view-note-1" })]).await;
}

#[tokio::test]
async fn fts_match_subscriptions_refresh_after_writes() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    sqlx::query("CREATE VIRTUAL TABLE docs_fts USING fts5(title, body)")
        .execute(&pool)
        .await
        .unwrap();

    let registration = subscribe(
        &runtime,
        "SELECT title FROM docs_fts WHERE docs_fts MATCH ? ORDER BY rowid",
        vec![json!("hello")],
        sink,
    )
    .await
    .unwrap();

    assert_eq!(
        registration.analysis,
        DependencyAnalysis::Reactive {
            targets: HashSet::from([DependencyTarget::VirtualTable("docs_fts".to_string())]),
        }
    );

    expect_empty_result(&events, 0).await;

    sqlx::query("INSERT INTO docs_fts (title, body) VALUES (?, ?)")
        .bind("hello world")
        .bind("greetings from fts")
        .execute(&pool)
        .await
        .unwrap();

    expect_result(&events, 1, vec![json!({ "title": "hello world" })]).await;
}

#[tokio::test]
async fn fts_shadow_table_changes_refresh_virtual_subscriptions() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    sqlx::query("CREATE VIRTUAL TABLE docs_fts USING fts5(title, body)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO docs_fts (title, body) VALUES (?, ?)")
        .bind("hello world")
        .bind("shadow table refresh")
        .execute(&pool)
        .await
        .unwrap();

    subscribe(
        &runtime,
        "SELECT title FROM docs_fts WHERE docs_fts MATCH ? ORDER BY rowid",
        vec![json!("hello")],
        sink,
    )
    .await
    .unwrap();

    expect_result(&events, 0, vec![json!({ "title": "hello world" })]).await;

    sqlx::query("INSERT INTO docs_fts(docs_fts) VALUES('rebuild')")
        .execute(&pool)
        .await
        .unwrap();

    expect_result(&events, 1, vec![json!({ "title": "hello world" })]).await;
}

#[tokio::test]
async fn ordinary_table_created_after_runtime_start_is_discovered() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    sqlx::query(
        "CREATE TABLE notes_added_later (id TEXT PRIMARY KEY NOT NULL, body TEXT NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let registration = subscribe(
        &runtime,
        "SELECT id FROM notes_added_later WHERE body IS NOT NULL ORDER BY id",
        Vec::new(),
        sink,
    )
    .await
    .unwrap();

    assert_eq!(
        registration.analysis,
        DependencyAnalysis::Reactive {
            targets: HashSet::from([DependencyTarget::Table("notes_added_later".to_string(),)]),
        }
    );

    expect_empty_result(&events, 0).await;

    sqlx::query("INSERT INTO notes_added_later (id, body) VALUES (?, ?)")
        .bind("late-note-1")
        .bind("hello")
        .execute(&pool)
        .await
        .unwrap();

    expect_result(&events, 1, vec![json!({ "id": "late-note-1" })]).await;
}

#[tokio::test]
async fn constant_selects_are_explicitly_non_reactive() {
    let (_dir, pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    let registration = subscribe(&runtime, "SELECT 1 AS value", Vec::new(), sink)
        .await
        .unwrap();

    assert!(matches!(
        &registration.analysis,
        DependencyAnalysis::NonReactive { .. }
    ));

    expect_result(&events, 0, vec![json!({ "value": 1 })]).await;

    insert_daily_note(&pool, "note-nonreactive", "2026-04-24", "user-1").await;

    expect_no_event(&events, 1).await;
}

#[tokio::test]
async fn extraction_failures_become_explicit_non_reactive_subscriptions() {
    let (_dir, _pool, runtime) = common::setup_runtime().await;
    let (sink, events) = TestSink::capture();

    let registration = subscribe(&runtime, "SELECT * FROM missing_table", Vec::new(), sink)
        .await
        .unwrap();

    assert!(matches!(
        &registration.analysis,
        DependencyAnalysis::NonReactive { .. }
    ));

    let analysis = runtime
        .dependency_analysis(&registration.id)
        .await
        .expect("subscription should exist");
    assert!(matches!(analysis, DependencyAnalysis::NonReactive { .. }));

    let _error = expect_error(&events, 0).await;
}

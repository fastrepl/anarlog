use std::sync::{Arc, Mutex};
use std::time::Duration;

use db_live_query::{DbRuntime, DependencyAnalysis, Error, QueryEventSink};
use hypr_db_core2::{DbOpenOptions, DbStorage, MigrationFailurePolicy};
use serde_json::json;

#[derive(Clone, Debug, PartialEq)]
enum TestEvent {
    Result(Vec<serde_json::Value>),
    Error(String),
}

#[derive(Clone)]
struct TestSink {
    events: Arc<Mutex<Vec<TestEvent>>>,
    fail_after: Option<usize>,
}

impl QueryEventSink for TestSink {
    fn send_result(&self, rows: Vec<serde_json::Value>) -> std::result::Result<(), String> {
        self.push(TestEvent::Result(rows))
    }

    fn send_error(&self, error: String) -> std::result::Result<(), String> {
        self.push(TestEvent::Error(error))
    }
}

impl TestSink {
    fn capture() -> (Self, Arc<Mutex<Vec<TestEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                events: Arc::clone(&events),
                fail_after: None,
            },
            events,
        )
    }

    fn fail_after(limit: usize) -> (Self, Arc<Mutex<Vec<TestEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                events: Arc::clone(&events),
                fail_after: Some(limit),
            },
            events,
        )
    }

    fn push(&self, event: TestEvent) -> std::result::Result<(), String> {
        let mut guard = self.events.lock().unwrap();
        if self.fail_after.is_some_and(|limit| guard.len() >= limit) {
            return Err("sink closed".to_string());
        }
        guard.push(event);
        Ok(())
    }
}

async fn next_event(
    events: &Arc<Mutex<Vec<TestEvent>>>,
    index: usize,
) -> anyhow::Result<TestEvent> {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if let Some(event) = events.lock().unwrap().get(index).cloned() {
                return event;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(anyhow::Error::from)
}

async fn setup_runtime() -> (tempfile::TempDir, sqlx::SqlitePool, DbRuntime<TestSink>) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let db = hypr_db_core2::Db3::open_with_migrate(
        DbOpenOptions {
            storage: DbStorage::Local(&db_path),
            cloudsync: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(4),
            migration_failure_policy: MigrationFailurePolicy::Fail,
        },
        |pool| Box::pin(hypr_db_app::migrate(pool)),
    )
    .await
    .unwrap();

    let pool = db.pool().as_ref().clone();

    (dir, pool, DbRuntime::new(db))
}

#[tokio::test]
async fn subscribe_sends_initial_result() {
    let (_dir, _pool, runtime) = setup_runtime().await;
    let (sink, events) = TestSink::capture();

    runtime
        .subscribe(
            "SELECT id, date FROM daily_notes ORDER BY id".to_string(),
            vec![],
            sink,
        )
        .await
        .unwrap();

    let event = next_event(&events, 0).await.unwrap();
    assert_eq!(event, TestEvent::Result(Vec::new()));
}

#[tokio::test]
async fn execute_binds_params_and_serializes_rows() {
    let (_dir, pool, runtime) = setup_runtime().await;

    sqlx::query(
        "CREATE TABLE query_values (
                id TEXT PRIMARY KEY NOT NULL,
                nullable_text TEXT,
                enabled BOOLEAN NOT NULL,
                visits INTEGER NOT NULL,
                ratio REAL NOT NULL,
                payload TEXT NOT NULL
            )",
    )
    .execute(&pool)
    .await
    .unwrap();

    let inserted = runtime
        .execute(
            "INSERT INTO query_values (id, nullable_text, enabled, visits, ratio, payload) VALUES (?, ?, ?, ?, ?, ?)"
                .to_string(),
            vec![
                json!("row-1"),
                serde_json::Value::Null,
                json!(true),
                json!(42),
                json!(1.5),
                json!({ "kind": "object" }),
            ],
        )
        .await
        .unwrap();
    assert!(inserted.is_empty());

    let rows = runtime
        .execute(
            "SELECT id, nullable_text, enabled, visits, ratio, payload FROM query_values"
                .to_string(),
            vec![],
        )
        .await
        .unwrap();

    assert_eq!(
        rows,
        vec![json!({
            "id": "row-1",
            "nullable_text": serde_json::Value::Null,
            "enabled": 1,
            "visits": 42,
            "ratio": 1.5,
            "payload": "{\"kind\":\"object\"}",
        })]
    );
}

#[tokio::test]
async fn dependent_writes_trigger_refresh() {
    let (_dir, pool, runtime) = setup_runtime().await;
    let (sink, events) = TestSink::capture();

    runtime
        .subscribe(
            "SELECT id, date FROM daily_notes ORDER BY id".to_string(),
            vec![],
            sink,
        )
        .await
        .unwrap();

    let _ = next_event(&events, 0).await.unwrap();

    sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
        .bind("note-1")
        .bind("2026-04-13")
        .bind("{}")
        .bind("user-1")
        .execute(&pool)
        .await
        .unwrap();

    let event = next_event(&events, 1).await.unwrap();
    let TestEvent::Result(rows) = event else {
        panic!("expected result event");
    };
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn unrelated_writes_do_not_trigger_refresh() {
    let (_dir, pool, runtime) = setup_runtime().await;
    let (sink, events) = TestSink::capture();

    sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
        .bind("note-seed")
        .bind("2026-04-12")
        .bind("{}")
        .bind("user-1")
        .execute(&pool)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    runtime
        .subscribe(
            "SELECT id, date FROM daily_notes ORDER BY id".to_string(),
            vec![],
            sink,
        )
        .await
        .unwrap();

    let _ = next_event(&events, 0).await.unwrap();

    sqlx::query(
        "INSERT INTO daily_summaries (id, daily_note_id, date, content, timeline_json, topics_json, status, source_cursor_ms, source_fingerprint, generation_error, generated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("summary-1")
    .bind("note-seed")
    .bind("2026-04-12")
    .bind("{}")
    .bind("[]")
    .bind("[]")
    .bind("ready")
    .bind(0_i64)
    .bind("")
    .bind("")
    .bind("2026-04-12T00:00:00Z")
    .execute(&pool)
    .await
    .unwrap();

    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(events.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn dependency_analysis_reports_reactive_tables() {
    let (_dir, _pool, runtime) = setup_runtime().await;
    let (sink, _events) = TestSink::capture();

    let registration = runtime
        .subscribe(
            "SELECT ds.id FROM daily_summaries ds JOIN daily_notes dn ON ds.daily_note_id = dn.id"
                .to_string(),
            vec![],
            sink,
        )
        .await
        .unwrap();

    let analysis = runtime
        .dependency_analysis(&registration.id)
        .await
        .expect("subscription should exist");

    assert_eq!(
        analysis,
        DependencyAnalysis::Reactive {
            tables: std::collections::HashSet::from([
                "daily_notes".to_string(),
                "daily_summaries".to_string(),
            ]),
        }
    );
}

#[tokio::test]
async fn unsubscribe_stops_future_events() {
    let (_dir, pool, runtime) = setup_runtime().await;
    let (sink, events) = TestSink::capture();

    let registration = runtime
        .subscribe(
            "SELECT id, date FROM daily_notes ORDER BY id".to_string(),
            vec![],
            sink,
        )
        .await
        .unwrap();

    let _ = next_event(&events, 0).await.unwrap();
    runtime.unsubscribe(&registration.id).await.unwrap();

    sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
        .bind("note-2")
        .bind("2026-04-14")
        .bind("{}")
        .bind("user-1")
        .execute(&pool)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(events.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn unsubscribe_returns_not_found_for_unknown_id() {
    let (_dir, _pool, runtime) = setup_runtime().await;

    let error = runtime.unsubscribe("missing").await.unwrap_err();
    assert!(matches!(error, Error::SubscriptionNotFound(id) if id == "missing"));
}

#[tokio::test]
async fn invalid_sql_sends_error_event() {
    let (_dir, _pool, runtime) = setup_runtime().await;
    let (sink, events) = TestSink::capture();

    runtime
        .subscribe("SELECT * FROM missing_table".to_string(), vec![], sink)
        .await
        .unwrap();

    let event = next_event(&events, 0).await.unwrap();
    assert!(matches!(event, TestEvent::Error(_)));
}

#[tokio::test]
async fn initial_sink_failure_rolls_back_registration() {
    let (_dir, _pool, runtime) = setup_runtime().await;
    let (sink, _events) = TestSink::fail_after(0);

    let error = runtime
        .subscribe(
            "SELECT id, date FROM daily_notes ORDER BY id".to_string(),
            vec![],
            sink,
        )
        .await
        .err()
        .expect("subscription should fail when the sink rejects the initial result");

    assert!(matches!(error, Error::Sink(message) if message == "sink closed"));
}

#[tokio::test]
async fn extraction_failures_become_explicit_non_reactive_subscriptions() {
    let (_dir, _pool, runtime) = setup_runtime().await;
    let (sink, events) = TestSink::capture();

    let registration = runtime
        .subscribe("SELECT * FROM missing_table".to_string(), vec![], sink)
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

    let event = next_event(&events, 0).await.unwrap();
    assert!(matches!(event, TestEvent::Error(_)));
}

#[tokio::test]
async fn stale_subscribers_are_removed_after_send_failures() {
    let (_dir, pool, runtime) = setup_runtime().await;
    let (sink, events) = TestSink::fail_after(1);

    let registration = runtime
        .subscribe(
            "SELECT id, date FROM daily_notes ORDER BY id".to_string(),
            vec![],
            sink,
        )
        .await
        .unwrap();

    let _ = next_event(&events, 0).await.unwrap();

    sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
        .bind("note-stale")
        .bind("2026-04-15")
        .bind("{}")
        .bind("user-1")
        .execute(&pool)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(150)).await;

    assert!(
        runtime
            .dependency_analysis(&registration.id)
            .await
            .is_none()
    );
    assert_eq!(events.lock().unwrap().len(), 1);
}

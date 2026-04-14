use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use db_live_query::{DbRuntime, DependencyAnalysis, DependencyTarget, Error, QueryEventSink};
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
    send_delay: Option<Duration>,
    send_block: Option<SendBlock>,
}

#[derive(Clone)]
struct SendBlock {
    event_index: usize,
    started: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
}

#[derive(Clone)]
struct SendBlockHandle {
    started: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
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
                send_delay: None,
                send_block: None,
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
                send_delay: None,
                send_block: None,
            },
            events,
        )
    }

    fn with_delay(delay: Duration) -> (Self, Arc<Mutex<Vec<TestEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                events: Arc::clone(&events),
                fail_after: None,
                send_delay: Some(delay),
                send_block: None,
            },
            events,
        )
    }

    fn with_blocked_send(
        event_index: usize,
    ) -> (Self, Arc<Mutex<Vec<TestEvent>>>, SendBlockHandle) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let started = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        (
            Self {
                events: Arc::clone(&events),
                fail_after: None,
                send_delay: None,
                send_block: Some(SendBlock {
                    event_index,
                    started: Arc::clone(&started),
                    release: Arc::clone(&release),
                }),
            },
            events,
            SendBlockHandle { started, release },
        )
    }

    fn push(&self, event: TestEvent) -> std::result::Result<(), String> {
        if let Some(block) = &self.send_block {
            let event_index = self.events.lock().unwrap().len();
            if event_index == block.event_index {
                block.started.store(true, Ordering::SeqCst);
                while !block.release.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
        if let Some(delay) = self.send_delay {
            std::thread::sleep(delay);
        }
        let mut guard = self.events.lock().unwrap();
        if self.fail_after.is_some_and(|limit| guard.len() >= limit) {
            return Err("sink closed".to_string());
        }
        guard.push(event);
        Ok(())
    }
}

impl SendBlockHandle {
    async fn wait_until_started(&self) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while !self.started.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("blocked send should start");
    }

    fn release(&self) {
        self.release.store(true, Ordering::SeqCst);
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

async fn wait_for_stable_event_count(
    events: &Arc<Mutex<Vec<TestEvent>>>,
    stable_for: Duration,
) -> usize {
    let mut last_len = events.lock().unwrap().len();
    loop {
        tokio::time::sleep(stable_for).await;
        let len = events.lock().unwrap().len();
        if len == last_len {
            return len;
        }
        last_len = len;
    }
}

async fn setup_runtime() -> (tempfile::TempDir, sqlx::SqlitePool, DbRuntime<TestSink>) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let db = hypr_db_core2::Db3::open_with_migrate(
        DbOpenOptions {
            storage: DbStorage::Local(&db_path),
            cloudsync_open_mode: hypr_db_core2::CloudsyncOpenMode::Disabled,
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

    (dir, pool, DbRuntime::new(std::sync::Arc::new(db)))
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initialization_defers_refresh_until_after_initial_snapshot() {
    let (_dir, pool, runtime) = setup_runtime().await;
    let (sink, events) = TestSink::with_delay(Duration::from_millis(50));

    let subscribe = tokio::spawn(async move {
        runtime
            .subscribe(
                "SELECT id, date FROM daily_notes ORDER BY id".to_string(),
                vec![],
                sink,
            )
            .await
    });

    tokio::time::sleep(Duration::from_millis(10)).await;

    sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
        .bind("note-init-race")
        .bind("2026-04-20")
        .bind("{}")
        .bind("user-1")
        .execute(&pool)
        .await
        .unwrap();

    subscribe.await.unwrap().unwrap();

    let initial = next_event(&events, 0).await.unwrap();
    assert_eq!(initial, TestEvent::Result(Vec::new()));

    let refresh = next_event(&events, 1).await.unwrap();
    let TestEvent::Result(rows) = refresh else {
        panic!("expected result event");
    };
    assert_eq!(rows.len(), 1);

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(events.lock().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initialization_suppresses_duplicate_catch_up_payloads() {
    let (_dir, pool, runtime) = setup_runtime().await;
    let (sink, events) = TestSink::with_delay(Duration::from_millis(50));

    let subscribe = tokio::spawn(async move {
        runtime
            .subscribe(
                "SELECT id FROM daily_notes WHERE date = ? ORDER BY id".to_string(),
                vec![json!("2026-04-21")],
                sink,
            )
            .await
    });

    tokio::time::sleep(Duration::from_millis(10)).await;

    sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
        .bind("note-nonmatching")
        .bind("2026-04-22")
        .bind("{}")
        .bind("user-1")
        .execute(&pool)
        .await
        .unwrap();

    subscribe.await.unwrap().unwrap();

    let initial = next_event(&events, 0).await.unwrap();
    assert_eq!(initial, TestEvent::Result(Vec::new()));

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(events.lock().unwrap().len(), 1);
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
async fn open_transactions_do_not_refresh_until_commit() {
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

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
        .bind("note-in-tx")
        .bind("2026-04-16")
        .bind("{}")
        .bind("user-1")
        .execute(&mut *tx)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(events.lock().unwrap().len(), 1);

    tx.commit().await.unwrap();

    let event = next_event(&events, 1).await.unwrap();
    let TestEvent::Result(rows) = event else {
        panic!("expected result event");
    };
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn rollback_after_write_does_not_refresh() {
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

    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(events.lock().unwrap().len(), 1);
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
async fn dependency_analysis_reports_reactive_targets() {
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
            targets: std::collections::HashSet::from([
                DependencyTarget::Table("daily_notes".to_string()),
                DependencyTarget::Table("daily_summaries".to_string()),
            ]),
        }
    );
}

#[tokio::test]
async fn fts_match_subscriptions_refresh_after_writes() {
    let (_dir, pool, runtime) = setup_runtime().await;
    let (sink, events) = TestSink::capture();

    sqlx::query("CREATE VIRTUAL TABLE docs_fts USING fts5(title, body)")
        .execute(&pool)
        .await
        .unwrap();

    let registration = runtime
        .subscribe(
            "SELECT title FROM docs_fts WHERE docs_fts MATCH ? ORDER BY rowid".to_string(),
            vec![json!("hello")],
            sink,
        )
        .await
        .unwrap();

    assert_eq!(
        registration.analysis,
        DependencyAnalysis::Reactive {
            targets: std::collections::HashSet::from([DependencyTarget::VirtualTable(
                "docs_fts".to_string(),
            )]),
        }
    );

    let initial = next_event(&events, 0).await.unwrap();
    assert_eq!(initial, TestEvent::Result(Vec::new()));

    sqlx::query("INSERT INTO docs_fts (title, body) VALUES (?, ?)")
        .bind("hello world")
        .bind("greetings from fts")
        .execute(&pool)
        .await
        .unwrap();

    let refresh = next_event(&events, 1).await.unwrap();
    let TestEvent::Result(rows) = refresh else {
        panic!("expected result event");
    };
    assert_eq!(rows, vec![json!({ "title": "hello world" })]);
}

#[tokio::test]
async fn virtual_table_created_after_runtime_start_is_discovered() {
    let (_dir, pool, runtime) = setup_runtime().await;
    let (sink, events) = TestSink::capture();

    sqlx::query("CREATE VIRTUAL TABLE docs_fts USING fts5(title, body)")
        .execute(&pool)
        .await
        .unwrap();

    runtime
        .subscribe(
            "SELECT rowid FROM docs_fts WHERE docs_fts MATCH ?".to_string(),
            vec![json!("reload")],
            sink,
        )
        .await
        .unwrap();

    let initial = next_event(&events, 0).await.unwrap();
    assert_eq!(initial, TestEvent::Result(Vec::new()));

    sqlx::query("INSERT INTO docs_fts (title, body) VALUES (?, ?)")
        .bind("reload")
        .bind("schema catalog refresh")
        .execute(&pool)
        .await
        .unwrap();

    let refresh = next_event(&events, 1).await.unwrap();
    let TestEvent::Result(rows) = refresh else {
        panic!("expected result event");
    };
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn unsupported_virtual_tables_are_explicitly_non_reactive() {
    let (_dir, pool, runtime) = setup_runtime().await;
    let (sink, events) = TestSink::capture();

    sqlx::query("CREATE VIRTUAL TABLE docs_rtree USING rtree(id, min_x, max_x)")
        .execute(&pool)
        .await
        .unwrap();

    let registration = runtime
        .subscribe(
            "SELECT id FROM docs_rtree ORDER BY id".to_string(),
            vec![],
            sink,
        )
        .await
        .unwrap();

    assert!(matches!(
        registration.analysis,
        DependencyAnalysis::NonReactive { .. }
    ));

    let initial = next_event(&events, 0).await.unwrap();
    assert_eq!(initial, TestEvent::Result(Vec::new()));

    sqlx::query("INSERT INTO docs_rtree (id, min_x, max_x) VALUES (?, ?, ?)")
        .bind(1_i64)
        .bind(0.0_f64)
        .bind(1.0_f64)
        .execute(&pool)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(events.lock().unwrap().len(), 1);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsubscribe_waits_for_in_flight_refresh_delivery() {
    let (_dir, pool, runtime) = setup_runtime().await;
    let (sink, events, send_block) = TestSink::with_blocked_send(1);

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
        .bind("note-blocked-refresh")
        .bind("2026-04-23")
        .bind("{}")
        .bind("user-1")
        .execute(&pool)
        .await
        .unwrap();

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

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(events.lock().unwrap().len(), 2);
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

#[tokio::test]
async fn lagged_broadcast_receiver_resyncs_and_keeps_dispatcher_alive() {
    let (_dir, pool, runtime) = setup_runtime().await;
    let (sink, events) = TestSink::with_delay(Duration::from_millis(5));

    runtime
        .subscribe(
            "SELECT id, date FROM daily_notes ORDER BY id".to_string(),
            vec![],
            sink,
        )
        .await
        .unwrap();

    let _ = next_event(&events, 0).await.unwrap();

    for idx in 0..320 {
        sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
            .bind(format!("note-lag-{idx}"))
            .bind("2026-04-18")
            .bind("{}")
            .bind(format!("user-lag-{idx}"))
            .execute(&pool)
            .await
            .unwrap();
    }

    let _stable_count = wait_for_stable_event_count(&events, Duration::from_millis(100)).await;

    let before = events.lock().unwrap().len();

    sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
        .bind("note-after-lag")
        .bind("2026-04-19")
        .bind("{}")
        .bind("user-after-lag")
        .execute(&pool)
        .await
        .unwrap();

    let event = next_event(&events, before).await.unwrap();
    let TestEvent::Result(rows) = event else {
        panic!("expected result event");
    };
    assert!(rows.len() >= 321);
}

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use db_reactive::{LiveQueryRuntime, QueryEventSink, SubscriptionRegistration};
use hypr_db_core::{DbOpenOptions, DbStorage};
use serde_json::{Value, json};

const LIVE_QUERY_TEST_MIGRATION_STEPS: &[hypr_db_migrate::MigrationStep] =
    &[hypr_db_migrate::MigrationStep {
        id: "20260415000000_live_query_test_schema",
        scope: hypr_db_migrate::MigrationScope::Plain,
        sql: include_str!("live_query_test_schema.sql"),
    }];

fn live_query_test_schema() -> hypr_db_migrate::DbSchema {
    hypr_db_migrate::DbSchema {
        steps: LIVE_QUERY_TEST_MIGRATION_STEPS,
        validate_cloudsync_table: |_| false,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TestEvent {
    Result(Vec<serde_json::Value>),
    Error(String),
}

pub type EventLog = Arc<Mutex<Vec<TestEvent>>>;
pub type TestRuntime = LiveQueryRuntime<TestSink>;

pub const EVENT_TIMEOUT: Duration = Duration::from_secs(1);
pub const QUIET_PERIOD: Duration = Duration::from_millis(150);

#[derive(Clone)]
pub struct TestSink {
    events: EventLog,
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
pub struct SendBlockHandle {
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
    pub fn capture() -> (Self, EventLog) {
        Self::with_options(None, None, None)
    }

    pub fn fail_after(limit: usize) -> (Self, EventLog) {
        Self::with_options(Some(limit), None, None)
    }

    pub fn with_delay(delay: Duration) -> (Self, EventLog) {
        Self::with_options(None, Some(delay), None)
    }

    pub fn with_blocked_send(event_index: usize) -> (Self, EventLog, SendBlockHandle) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let started = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let send_block = SendBlock {
            event_index,
            started: Arc::clone(&started),
            release: Arc::clone(&release),
        };

        (
            Self::new(events.clone(), None, None, Some(send_block)),
            events,
            SendBlockHandle { started, release },
        )
    }

    fn with_options(
        fail_after: Option<usize>,
        send_delay: Option<Duration>,
        send_block: Option<SendBlock>,
    ) -> (Self, EventLog) {
        let events = Arc::new(Mutex::new(Vec::new()));
        (
            Self::new(events.clone(), fail_after, send_delay, send_block),
            events,
        )
    }

    fn new(
        events: EventLog,
        fail_after: Option<usize>,
        send_delay: Option<Duration>,
        send_block: Option<SendBlock>,
    ) -> Self {
        Self {
            events,
            fail_after,
            send_delay,
            send_block,
        }
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
    pub async fn wait_until_started(&self) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while !self.started.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("blocked send should start");
    }

    pub fn release(&self) {
        self.release.store(true, Ordering::SeqCst);
    }
}

pub async fn next_event(
    events: &EventLog,
    index: usize,
    timeout: Duration,
) -> anyhow::Result<TestEvent> {
    tokio::time::timeout(timeout, async {
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

pub async fn assert_no_event(events: &EventLog, index: usize, timeout: Duration) {
    let result = tokio::time::timeout(timeout, async {
        loop {
            if let Some(event) = events.lock().unwrap().get(index).cloned() {
                return event;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;

    assert!(
        result.is_err(),
        "unexpected event at index {index}: {:?}",
        result.unwrap()
    );
}

pub async fn wait_for_stable_event_count(events: &EventLog, stable_for: Duration) -> usize {
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

pub async fn setup_runtime() -> (tempfile::TempDir, sqlx::SqlitePool, TestRuntime) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let db = hypr_db_core::Db::open(DbOpenOptions {
        storage: DbStorage::Local(&db_path),
        cloudsync_enabled: false,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await
    .unwrap();
    hypr_db_migrate::migrate(&db, live_query_test_schema())
        .await
        .unwrap();

    let pool = db.pool().clone();

    (dir, pool, LiveQueryRuntime::new(std::sync::Arc::new(db)))
}

pub async fn expect_result(events: &EventLog, index: usize, expected: Vec<Value>) {
    assert_eq!(
        next_event(events, index, EVENT_TIMEOUT).await.unwrap(),
        TestEvent::Result(expected)
    );
}

pub async fn expect_empty_result(events: &EventLog, index: usize) {
    expect_result(events, index, Vec::new()).await;
}

pub async fn expect_error(events: &EventLog, index: usize) -> String {
    match next_event(events, index, EVENT_TIMEOUT).await.unwrap() {
        TestEvent::Error(error) => error,
        TestEvent::Result(rows) => panic!("expected error event, got result: {rows:?}"),
    }
}

pub async fn next_result_rows(events: &EventLog, index: usize) -> Vec<Value> {
    match next_event(events, index, EVENT_TIMEOUT).await.unwrap() {
        TestEvent::Result(rows) => rows,
        TestEvent::Error(error) => panic!("expected result event, got error: {error}"),
    }
}

pub async fn expect_no_event(events: &EventLog, index: usize) {
    assert_no_event(events, index, QUIET_PERIOD).await;
}

pub async fn wait_until_subscription_removed(runtime: &TestRuntime, subscription_id: &str) {
    tokio::time::timeout(EVENT_TIMEOUT, async {
        loop {
            if runtime.dependency_analysis(subscription_id).await.is_none() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("subscription should be removed");
}

pub fn all_daily_notes_sql() -> String {
    "SELECT id, date FROM daily_notes ORDER BY id".to_string()
}

pub fn daily_note_by_id_sql() -> String {
    "SELECT id, date FROM daily_notes WHERE id = ?".to_string()
}

pub fn all_daily_summaries_sql() -> String {
    "SELECT id, date FROM daily_summaries ORDER BY id".to_string()
}

pub async fn subscribe(
    runtime: &TestRuntime,
    sql: impl Into<String>,
    params: Vec<Value>,
    sink: TestSink,
) -> db_reactive::Result<SubscriptionRegistration> {
    runtime.subscribe(sql.into(), params, sink).await
}

pub async fn subscribe_all_daily_notes(
    runtime: &TestRuntime,
    sink: TestSink,
) -> db_reactive::Result<SubscriptionRegistration> {
    subscribe(runtime, all_daily_notes_sql(), Vec::new(), sink).await
}

pub async fn subscribe_daily_note_by_id(
    runtime: &TestRuntime,
    id: &str,
    sink: TestSink,
) -> db_reactive::Result<SubscriptionRegistration> {
    subscribe(runtime, daily_note_by_id_sql(), vec![json!(id)], sink).await
}

pub async fn subscribe_all_daily_summaries(
    runtime: &TestRuntime,
    sink: TestSink,
) -> db_reactive::Result<SubscriptionRegistration> {
    subscribe(runtime, all_daily_summaries_sql(), Vec::new(), sink).await
}

pub async fn insert_daily_note(pool: &sqlx::SqlitePool, id: &str, date: &str, user_id: &str) {
    sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
        .bind(id)
        .bind(date)
        .bind("{}")
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
}

pub async fn insert_daily_summary(
    pool: &sqlx::SqlitePool,
    id: &str,
    daily_note_id: &str,
    date: &str,
) {
    sqlx::query(
        "INSERT INTO daily_summaries (id, daily_note_id, date, content, timeline_json, topics_json, status, source_cursor_ms, source_fingerprint, generation_error, generated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(daily_note_id)
    .bind(date)
    .bind("{}")
    .bind("[]")
    .bind("[]")
    .bind("ready")
    .bind(0_i64)
    .bind("")
    .bind("")
    .bind(format!("{date}T00:00:00Z"))
    .execute(pool)
    .await
    .unwrap();
}

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use db_live_query::{DbRuntime, QueryEventSink};
use hypr_db_core2::{DbOpenOptions, DbStorage, MigrationFailurePolicy};

#[derive(Clone, Debug, PartialEq)]
pub enum TestEvent {
    Result(Vec<serde_json::Value>),
    Error(String),
}

#[derive(Clone)]
pub struct TestSink {
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
    pub fn capture() -> (Self, Arc<Mutex<Vec<TestEvent>>>) {
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

    pub fn fail_after(limit: usize) -> (Self, Arc<Mutex<Vec<TestEvent>>>) {
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

    pub fn with_delay(delay: Duration) -> (Self, Arc<Mutex<Vec<TestEvent>>>) {
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

    pub fn with_blocked_send(
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
    events: &Arc<Mutex<Vec<TestEvent>>>,
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

pub async fn wait_for_stable_event_count(
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

pub async fn setup_runtime() -> (tempfile::TempDir, sqlx::SqlitePool, DbRuntime<TestSink>) {
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

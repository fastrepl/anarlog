#![forbid(unsafe_code)]

mod explain;

use std::collections::HashSet;
use std::sync::Arc;

use hypr_db_core2::Db3;
use hypr_db_watch::{TableDeps, WatchId};
use sqlx::{Column, Row, TypeInfo, ValueRef};
use tokio::sync::watch;

pub use explain::extract_tables;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error("subscription not found: {0}")]
    SubscriptionNotFound(String),
    #[error("failed to send query event: {0}")]
    Sink(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait QueryEventSink: Clone + Send + 'static {
    fn send_result(&self, rows: Vec<serde_json::Value>) -> std::result::Result<(), String>;
    fn send_error(&self, error: String) -> std::result::Result<(), String>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyAnalysis {
    Reactive { tables: HashSet<String> },
    NonReactive { reason: String },
}

impl DependencyAnalysis {
    pub fn is_reactive(&self) -> bool {
        matches!(self, Self::Reactive { .. })
    }
}

struct Subscription<S> {
    id: String,
    sql: String,
    params: Vec<serde_json::Value>,
    sink: S,
}

enum SubscriptionSlot {
    Reactive(WatchId),
    NonReactive,
}

struct Inner<S> {
    deps: TableDeps,
    ids: std::collections::HashMap<String, SubscriptionSlot>,
    subscriptions: std::collections::HashMap<WatchId, Subscription<S>>,
    non_reactive: std::collections::HashMap<String, String>,
}

impl<S> Default for Inner<S> {
    fn default() -> Self {
        Self {
            deps: TableDeps::default(),
            ids: std::collections::HashMap::new(),
            subscriptions: std::collections::HashMap::new(),
            non_reactive: std::collections::HashMap::new(),
        }
    }
}

pub struct DbRuntime<S> {
    db: Arc<Db3>,
    inner: Arc<tokio::sync::Mutex<Inner<S>>>,
    shutdown_tx: watch::Sender<bool>,
    dispatcher: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

pub struct SubscriptionRegistration {
    pub id: String,
    pub analysis: DependencyAnalysis,
}

impl<S: QueryEventSink> DbRuntime<S> {
    pub fn new(db: Db3) -> Self {
        let db = Arc::new(db);
        let inner = Arc::new(tokio::sync::Mutex::<Inner<S>>::new(Inner::default()));
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let mut change_rx = db.subscribe_table_changes();
        let dispatcher_inner = Arc::clone(&inner);
        let dispatcher_db = Arc::clone(&db);

        let dispatcher = tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    change = change_rx.recv() => {
                        let Ok(first_change) = change else {
                            break;
                        };

                        let mut changed_tables = HashSet::from([first_change.table]);
                        while let Ok(next_change) = change_rx.try_recv() {
                            changed_tables.insert(next_change.table);
                        }

                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                        let jobs = collect_jobs(&dispatcher_inner, &changed_tables).await;
                        if jobs.is_empty() {
                            continue;
                        }

                        let mut stale = Vec::new();
                        for job in jobs {
                            let stale_watch_id = job.watch_id;
                            let send_result = match run_query(&dispatcher_db, &job.sql, &job.params).await {
                                Ok(rows) => job.sink.send_result(rows),
                                Err(error) => job.sink.send_error(error.to_string()),
                            };

                            if send_result.is_err() {
                                stale.push(stale_watch_id);
                            }
                        }

                        if !stale.is_empty() {
                            remove_stale(&dispatcher_inner, &stale).await;
                        }
                    }
                }
            }
        });

        Self {
            db,
            inner,
            shutdown_tx,
            dispatcher: std::sync::Mutex::new(Some(dispatcher)),
        }
    }

    pub fn pool(&self) -> &sqlx::SqlitePool {
        self.db.pool()
    }

    pub async fn execute(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>> {
        run_query(&self.db, &sql, &params).await.map_err(Into::into)
    }

    pub async fn subscribe(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
        sink: S,
    ) -> Result<SubscriptionRegistration> {
        let registration = self
            .register_subscription(sql.clone(), params.clone(), sink.clone())
            .await;

        let event_result = match run_query(&self.db, &sql, &params).await {
            Ok(rows) => sink.send_result(rows),
            Err(error) => sink.send_error(error.to_string()),
        };

        if let Err(error) = event_result {
            self.unregister_registered(&registration.id).await;
            return Err(Error::Sink(error));
        }

        Ok(registration)
    }

    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<()> {
        let removed = self.unregister_registered(subscription_id).await;
        if removed {
            Ok(())
        } else {
            Err(Error::SubscriptionNotFound(subscription_id.to_string()))
        }
    }

    pub async fn dependency_analysis(&self, subscription_id: &str) -> Option<DependencyAnalysis> {
        let inner = self.inner.lock().await;
        match inner.ids.get(subscription_id) {
            Some(SubscriptionSlot::Reactive(watch_id)) => {
                inner.subscriptions.get(watch_id).map(|_| {
                    let tables = inner.deps.tables_for(*watch_id).unwrap_or_default();
                    DependencyAnalysis::Reactive { tables }
                })
            }
            Some(SubscriptionSlot::NonReactive) => {
                inner.non_reactive.get(subscription_id).map(|reason| {
                    DependencyAnalysis::NonReactive {
                        reason: reason.clone(),
                    }
                })
            }
            None => None,
        }
    }

    async fn register_subscription(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
        sink: S,
    ) -> SubscriptionRegistration {
        let subscription_id = uuid::Uuid::new_v4().to_string();
        let analysis = match extract_tables(self.db.pool(), &sql).await {
            Ok(tables) => DependencyAnalysis::Reactive { tables },
            Err(error) => DependencyAnalysis::NonReactive {
                reason: error.to_string(),
            },
        };

        let subscription = Subscription {
            id: subscription_id.clone(),
            sql,
            params,
            sink,
        };

        let mut inner = self.inner.lock().await;
        match &analysis {
            DependencyAnalysis::Reactive { tables } => {
                let watch_id = inner.deps.register(tables.clone());
                inner.ids.insert(
                    subscription_id.clone(),
                    SubscriptionSlot::Reactive(watch_id),
                );
                inner.subscriptions.insert(watch_id, subscription);
            }
            DependencyAnalysis::NonReactive { reason } => {
                inner
                    .ids
                    .insert(subscription_id.clone(), SubscriptionSlot::NonReactive);
                inner
                    .non_reactive
                    .insert(subscription_id.clone(), reason.clone());
                drop(subscription);
            }
        }

        SubscriptionRegistration {
            id: subscription_id,
            analysis,
        }
    }

    async fn unregister_registered(&self, subscription_id: &str) -> bool {
        let mut inner = self.inner.lock().await;
        match inner.ids.remove(subscription_id) {
            Some(SubscriptionSlot::Reactive(watch_id)) => {
                inner.subscriptions.remove(&watch_id);
                inner.deps.unregister(watch_id);
                true
            }
            Some(SubscriptionSlot::NonReactive) => {
                inner.non_reactive.remove(subscription_id);
                true
            }
            None => false,
        }
    }
}

impl<S> Drop for DbRuntime<S> {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
        if let Some(dispatcher) = self.dispatcher.lock().unwrap().take() {
            dispatcher.abort();
        }
    }
}

#[derive(Clone)]
struct RefreshJob<S> {
    watch_id: WatchId,
    sql: String,
    params: Vec<serde_json::Value>,
    sink: S,
}

async fn collect_jobs<S: QueryEventSink>(
    inner: &Arc<tokio::sync::Mutex<Inner<S>>>,
    changed_tables: &HashSet<String>,
) -> Vec<RefreshJob<S>> {
    let changed_refs = changed_tables
        .iter()
        .map(std::string::String::as_str)
        .collect::<Vec<_>>();

    let guard = inner.lock().await;
    guard
        .deps
        .affected(&changed_refs)
        .into_iter()
        .filter_map(|watch_id| {
            guard
                .subscriptions
                .get(&watch_id)
                .map(|subscription| RefreshJob {
                    watch_id,
                    sql: subscription.sql.clone(),
                    params: subscription.params.clone(),
                    sink: subscription.sink.clone(),
                })
        })
        .collect()
}

async fn remove_stale<S: QueryEventSink>(
    inner: &Arc<tokio::sync::Mutex<Inner<S>>>,
    stale: &[WatchId],
) {
    let mut guard = inner.lock().await;
    for watch_id in stale {
        if let Some(subscription) = guard.subscriptions.remove(watch_id) {
            guard.ids.remove(&subscription.id);
        }
        guard.deps.unregister(*watch_id);
    }
}

async fn run_query(
    db: &Db3,
    sql: &str,
    params: &[serde_json::Value],
) -> std::result::Result<Vec<serde_json::Value>, sqlx::Error> {
    let mut query = sqlx::query(sql);
    for param in params {
        query = match param {
            serde_json::Value::Null => query.bind(None::<String>),
            serde_json::Value::Bool(value) => query.bind(*value),
            serde_json::Value::Number(value) => {
                if let Some(integer) = value.as_i64() {
                    query.bind(integer)
                } else {
                    query.bind(value.as_f64().unwrap_or_default())
                }
            }
            serde_json::Value::String(value) => query.bind(value.clone()),
            other => query.bind(other.to_string()),
        };
    }

    let rows = query.fetch_all(db.pool().as_ref()).await?;
    Ok(rows.iter().map(row_to_json).collect())
}

fn row_to_json(row: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (index, column) in row.columns().iter().enumerate() {
        let value = match row.try_get_raw(index) {
            Ok(raw) if !raw.is_null() => match raw.type_info().name() {
                "TEXT" => row
                    .get::<Option<String>, _>(index)
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null),
                "INTEGER" | "INT" | "BOOLEAN" => row
                    .get::<Option<i64>, _>(index)
                    .map(serde_json::Value::from)
                    .unwrap_or(serde_json::Value::Null),
                "REAL" => row
                    .get::<Option<f64>, _>(index)
                    .map(serde_json::Value::from)
                    .unwrap_or(serde_json::Value::Null),
                "BLOB" => row
                    .get::<Option<Vec<u8>>, _>(index)
                    .map(serde_json::Value::from)
                    .unwrap_or(serde_json::Value::Null),
                _ => row
                    .get::<Option<String>, _>(index)
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null),
            },
            _ => serde_json::Value::Null,
        };
        map.insert(column.name().to_string(), value);
    }

    serde_json::Value::Object(map)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use hypr_db_core2::{DbOpenOptions, DbStorage, MigrationFailurePolicy};

    use super::*;

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

    async fn setup_runtime() -> (tempfile::TempDir, DbRuntime<TestSink>) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = Db3::open_with_migrate(
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

        (dir, DbRuntime::new(db))
    }

    #[tokio::test]
    async fn subscribe_sends_initial_result() {
        let (_dir, runtime) = setup_runtime().await;
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
    async fn dependent_writes_trigger_refresh() {
        let (_dir, runtime) = setup_runtime().await;
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
            .execute(runtime.pool())
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
        let (_dir, runtime) = setup_runtime().await;
        let (sink, events) = TestSink::capture();

        sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
            .bind("note-seed")
            .bind("2026-04-12")
            .bind("{}")
            .bind("user-1")
            .execute(runtime.pool())
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
        .execute(runtime.pool())
        .await
        .unwrap();

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn unsubscribe_stops_future_events() {
        let (_dir, runtime) = setup_runtime().await;
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
            .execute(runtime.pool())
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn invalid_sql_sends_error_event() {
        let (_dir, runtime) = setup_runtime().await;
        let (sink, events) = TestSink::capture();

        runtime
            .subscribe("SELECT * FROM missing_table".to_string(), vec![], sink)
            .await
            .unwrap();

        let event = next_event(&events, 0).await.unwrap();
        assert!(matches!(event, TestEvent::Error(_)));
    }

    #[tokio::test]
    async fn extraction_failures_become_explicit_non_reactive_subscriptions() {
        let (_dir, runtime) = setup_runtime().await;
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
        let (_dir, runtime) = setup_runtime().await;
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
            .execute(runtime.pool())
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
}

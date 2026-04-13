#![forbid(unsafe_code)]

mod error;
mod explain;
mod query;
mod watch;

use std::collections::HashSet;
use std::sync::Arc;

use hypr_db_core2::Db3;
use tokio::sync::watch as tokio_watch;
use watch::{TableDeps, WatchId};

pub use error::{Error, Result};
pub use explain::extract_tables;

use query::run_query;

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
    shutdown_tx: tokio_watch::Sender<bool>,
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
        let (shutdown_tx, mut shutdown_rx) = tokio_watch::channel(false);
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

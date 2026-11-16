use std::{
    collections::HashSet,
    io,
    sync::{Arc, Mutex, atomic::Ordering},
};

use anlg_db_core::Db;
use anlg_db_reactive::{LiveQueryRuntime, QueryEventSink, SubscriptionRegistration};
use serde_json::Value;
use tokio::sync::{OwnedSemaphorePermit, mpsc, watch};

use crate::{
    DocumentSnapshot, MAX_DOCUMENT_BYTES, Reply, Result, RuntimeHandle, ServiceError, SessionId,
    types::failure,
};

pub const MAX_WATCH_ROWS: usize = 1000;

struct SizeBound(usize);

impl io::Write for SizeBound {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 += bytes.len();
        if self.0 > MAX_DOCUMENT_BYTES {
            return Err(io::Error::other("Watch exceeds 16 MiB"));
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) struct WatchRegistry {
    runtime: LiveQueryRuntime<Sink>,
    ids: tokio::sync::Mutex<HashSet<String>>,
    executor: tokio::runtime::Handle,
}

impl WatchRegistry {
    pub(crate) fn new(db: Arc<Db>) -> Self {
        Self {
            runtime: LiveQueryRuntime::new(db),
            ids: tokio::sync::Mutex::new(HashSet::new()),
            executor: tokio::runtime::Handle::current(),
        }
    }

    async fn unsubscribe(&self, id: &str) -> Result<()> {
        let mut ids = self.ids.lock().await;
        if ids.contains(id) {
            // A failed sink may already have removed the subscription.
            if self.runtime.dependency_analysis(id).await.is_some() {
                self.runtime.unsubscribe(id).await.map_err(failure)?;
            }
            ids.remove(id);
        }
        Ok(())
    }

    pub(crate) async fn close(&self) {
        let mut ids = self.ids.lock().await;
        for id in ids.drain() {
            let _ = self.runtime.unsubscribe(&id).await;
        }
    }
}

#[derive(Clone, Debug)]
pub struct WatchSnapshot {
    pub sequence: u64,
    pub rows: Arc<[Value]>,
}

#[derive(Clone)]
struct Sink {
    snapshots: watch::Sender<WatchSnapshot>,
    errors: mpsc::Sender<ServiceError>,
    terminal_error: Arc<Mutex<Option<ServiceError>>>,
    metrics: Arc<crate::metrics::Metrics>,
    invalidate_on_refresh: bool,
}

impl QueryEventSink for Sink {
    fn send_result(&self, rows: Vec<Value>) -> std::result::Result<(), String> {
        if self.snapshots.receiver_count() == 0 {
            return Err("Watch receiver closed".into());
        }
        let mut size = SizeBound(0);
        if rows.len() > MAX_WATCH_ROWS || serde_json::to_writer(&mut size, &rows).is_err() {
            let error = ServiceError::Unsupported(
                "Watch exceeds 1000 rows or 16 MiB; paginate the query".into(),
            );
            *self.terminal_error.lock().map_err(|e| e.to_string())? = Some(error);
            self.snapshots
                .send_modify(|snapshot| snapshot.sequence += 1);
            return Err("Watch snapshot exceeds row bound".into());
        }
        self.snapshots.send_if_modified(|snapshot| {
            if !self.invalidate_on_refresh
                && snapshot.sequence > 0
                && snapshot.rows.as_ref() == rows.as_slice()
            {
                return false;
            }
            self.metrics.snapshot_count.fetch_add(1, Ordering::Relaxed);
            self.metrics
                .snapshot_rows
                .fetch_add(rows.len() as u64, Ordering::Relaxed);
            self.metrics
                .snapshot_bytes
                .fetch_add(size.0 as u64, Ordering::Relaxed);
            snapshot.sequence += 1;
            snapshot.rows = rows.into();
            true
        });
        Ok(())
    }

    fn send_error(&self, error: String) -> std::result::Result<(), String> {
        let error = ServiceError::Failed(error.into());
        if self.errors.try_send(error.clone()).is_err() {
            *self.terminal_error.lock().map_err(|e| e.to_string())? = Some(error);
            self.snapshots
                .send_modify(|snapshot| snapshot.sequence += 1);
            return Err("Watch error queue overflow; subscription terminated".into());
        }
        self.snapshots
            .send_modify(|snapshot| snapshot.sequence += 1);
        Ok(())
    }
}

pub struct QueryWatch {
    pub registration: SubscriptionRegistration,
    pub snapshots: watch::Receiver<WatchSnapshot>,
    pub errors: mpsc::Receiver<ServiceError>,
    terminal_error: Arc<Mutex<Option<ServiceError>>>,
    registry: Arc<WatchRegistry>,
    slot: Option<OwnedSemaphorePermit>,
}

impl QueryWatch {
    pub fn terminal_error(&self) -> Option<ServiceError> {
        self.terminal_error
            .lock()
            .ok()
            .and_then(|error| error.clone())
    }

    pub async fn unsubscribe(mut self) -> Result<()> {
        self.registry.unsubscribe(&self.registration.id).await?;
        self.slot.take();
        Ok(())
    }
}

impl Drop for QueryWatch {
    fn drop(&mut self) {
        let registry = self.registry.clone();
        let id = self.registration.id.clone();
        let slot = self.slot.take();
        self.registry.executor.spawn(async move {
            let _ = registry.unsubscribe(&id).await;
            drop(slot);
        });
    }
}

pub struct DocumentWatch(pub QueryWatch);

impl DocumentWatch {
    /// Call on a background executor; decoding can copy a large document.
    pub fn snapshot(&self) -> Result<Option<DocumentSnapshot>> {
        if let Some(error) = self.0.terminal_error() {
            return Err(error);
        }
        self.0
            .snapshots
            .borrow()
            .rows
            .first()
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(failure)
    }

    pub async fn unsubscribe(self) -> Result<()> {
        self.0.unsubscribe().await
    }
}

impl RuntimeHandle {
    pub fn watch_document(&self, id: SessionId) -> Result<Reply<QueryWatch>> {
        self.watch_query(
            "SELECT id, session_id, body_format, body, updated_at FROM session_documents WHERE session_id = ? AND kind = 'note' AND deleted_at IS NULL ORDER BY CASE WHEN id = ? THEN 0 ELSE 1 END, created_at, id LIMIT 1".into(),
            vec![serde_json::json!(id), serde_json::json!(id)],
        )
    }

    pub fn watch_library(&self) -> Result<Reply<QueryWatch>> {
        self.watch_query_policy(
            "SELECT id, title, updated_at,
                EXISTS(SELECT 1 FROM tags LIMIT 1) AS has_tags,
                EXISTS(SELECT 1 FROM session_tags LIMIT 1) AS has_session_tags
             FROM sessions WHERE deleted_at IS NULL ORDER BY updated_at DESC, id DESC LIMIT 1"
                .into(),
            vec![],
            true,
        )
    }

    /// Observes pooled writes. Close and re-register after an external writer or sync restore;
    /// SQLite pool hooks do not observe arbitrary writes from another process.
    pub fn watch_query(&self, sql: String, params: Vec<Value>) -> Result<Reply<QueryWatch>> {
        self.watch_query_policy(sql, params, false)
    }

    fn watch_query_policy(
        &self,
        sql: String,
        params: Vec<Value>,
        invalidate_on_refresh: bool,
    ) -> Result<Reply<QueryWatch>> {
        self.submit(move |services| async move {
            let slot = services
                .watch_slots
                .try_acquire_owned()
                .map_err(|_| ServiceError::Busy)?;
            let (snapshots, receiver) = watch::channel(WatchSnapshot {
                sequence: 0,
                rows: Arc::from([]),
            });
            let (errors, error_receiver) = mpsc::channel(8);
            let terminal_error = Arc::new(Mutex::new(None));
            let registry = services.watches;
            let registration = registry
                .runtime
                .subscribe(
                    sql,
                    params,
                    Sink {
                        snapshots,
                        errors,
                        terminal_error: terminal_error.clone(),
                        metrics: services.metrics,
                        invalidate_on_refresh,
                    },
                )
                .await
                .map_err(failure)?;
            registry.ids.lock().await.insert(registration.id.clone());
            Ok(QueryWatch {
                registration,
                snapshots: receiver,
                errors: error_receiver,
                terminal_error,
                registry,
                slot: Some(slot),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_empty_refresh_does_not_overtake_terminal_error_notification() {
        let (snapshots, mut receiver) = watch::channel(WatchSnapshot {
            sequence: 0,
            rows: Arc::from([]),
        });
        let (errors, _errors) = mpsc::channel(8);
        let sink = Sink {
            snapshots,
            errors,
            terminal_error: Arc::new(Mutex::new(None)),
            metrics: Arc::new(crate::metrics::Metrics::default()),
            invalidate_on_refresh: false,
        };
        sink.send_result(vec![]).unwrap();
        assert_eq!(receiver.borrow_and_update().sequence, 1);
        sink.send_result(vec![]).unwrap();
        assert!(!receiver.has_changed().unwrap());
        assert_eq!(sink.metrics.snapshot_count.load(Ordering::Relaxed), 1);
        assert!(
            sink.send_result(vec![serde_json::json!({
                "title": "x".repeat(MAX_DOCUMENT_BYTES)
            })])
            .is_err()
        );
        assert!(receiver.has_changed().unwrap());
        assert!(matches!(
            *sink.terminal_error.lock().unwrap(),
            Some(ServiceError::Unsupported(_))
        ));
        assert!(receiver.borrow().rows.is_empty());
    }
}

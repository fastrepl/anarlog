use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteOperation, SqlitePoolOptions};
use tokio::sync::broadcast;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TableChangeKind {
    Insert,
    Update,
    Delete,
}

/// Best-effort table-level mutation signal emitted for writes observed on pooled SQLite
/// connections created by this crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableChange {
    pub table: String,
    pub kind: TableChangeKind,
    pub seq: u64,
}

#[derive(Clone, Debug)]
pub struct DbPool {
    pool: SqlitePool,
    table_change_tx: Arc<broadcast::Sender<TableChange>>,
    change_tracker: Arc<ChangeTracker>,
}

impl DbPool {
    /// Subscribe to best-effort table-level change notifications for writes observed through
    /// this pool's physical SQLite connections.
    pub fn subscribe_table_changes(&self) -> broadcast::Receiver<TableChange> {
        self.table_change_tx.subscribe()
    }

    pub fn current_table_change_seq(&self) -> u64 {
        self.change_tracker.current_seq()
    }

    pub fn latest_table_change_seq(&self, table: &str) -> Option<u64> {
        self.change_tracker.latest_table_seq(table)
    }

    pub async fn close(self) {
        self.pool.close().await;
    }
}

impl AsRef<SqlitePool> for DbPool {
    fn as_ref(&self) -> &SqlitePool {
        &self.pool
    }
}

impl Deref for DbPool {
    type Target = SqlitePool;

    fn deref(&self) -> &Self::Target {
        &self.pool
    }
}

pub(crate) async fn connect_pool(
    connect_options: SqliteConnectOptions,
    max_connections: Option<u32>,
) -> Result<DbPool, sqlx::Error> {
    let (table_change_tx, _) = broadcast::channel(256);
    let table_change_tx = Arc::new(table_change_tx);
    let change_tracker = Arc::new(ChangeTracker::default());

    let callback_tx = Arc::clone(&table_change_tx);
    let callback_tracker = Arc::clone(&change_tracker);
    let mut pool_options = SqlitePoolOptions::new().after_connect(move |conn, _| {
        let callback_tx = Arc::clone(&callback_tx);
        let callback_tracker = Arc::clone(&callback_tracker);
        Box::pin(async move {
            let mut handle = conn.lock_handle().await?;
            let hook_state = Arc::new(HookState::new(callback_tx, callback_tracker));

            let update_state = Arc::clone(&hook_state);
            handle.set_update_hook(move |update| {
                if let Some(kind) = table_change_kind(update.operation) {
                    update_state.record(update.table, kind);
                }
            });

            let commit_state = Arc::clone(&hook_state);
            handle.set_commit_hook(move || {
                commit_state.flush();
                true
            });

            handle.set_rollback_hook(move || {
                hook_state.clear();
            });

            Ok(())
        })
    });

    if let Some(max_connections) = max_connections {
        pool_options = pool_options.max_connections(max_connections);
    }

    let pool = pool_options.connect_with(connect_options).await?;
    Ok(DbPool {
        pool,
        table_change_tx,
        change_tracker,
    })
}

fn table_change_kind(operation: SqliteOperation) -> Option<TableChangeKind> {
    match operation {
        SqliteOperation::Insert => Some(TableChangeKind::Insert),
        SqliteOperation::Update => Some(TableChangeKind::Update),
        SqliteOperation::Delete => Some(TableChangeKind::Delete),
        SqliteOperation::Unknown(_) => None,
    }
}

#[derive(Debug)]
struct HookState {
    pending: std::sync::Mutex<HashMap<String, TableChangeKind>>,
    tx: Arc<broadcast::Sender<TableChange>>,
    change_tracker: Arc<ChangeTracker>,
}

impl HookState {
    fn new(tx: Arc<broadcast::Sender<TableChange>>, change_tracker: Arc<ChangeTracker>) -> Self {
        Self {
            pending: std::sync::Mutex::new(HashMap::new()),
            tx,
            change_tracker,
        }
    }

    fn record(&self, table: &str, kind: TableChangeKind) {
        self.pending.lock().unwrap().insert(table.to_string(), kind);
    }

    fn flush(&self) {
        let pending = std::mem::take(&mut *self.pending.lock().unwrap());
        if pending.is_empty() {
            return;
        }

        let seq = self.change_tracker.next_seq();
        self.change_tracker.record_committed(&pending, seq);
        for (table, kind) in pending {
            let _ = self.tx.send(TableChange { table, kind, seq });
        }
    }

    fn clear(&self) {
        self.pending.lock().unwrap().clear();
    }
}

#[derive(Debug, Default)]
struct ChangeTracker {
    current_seq: AtomicU64,
    latest_by_table: std::sync::Mutex<HashMap<String, u64>>,
}

impl ChangeTracker {
    fn next_seq(&self) -> u64 {
        self.current_seq.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn current_seq(&self) -> u64 {
        self.current_seq.load(Ordering::SeqCst)
    }

    fn latest_table_seq(&self, table: &str) -> Option<u64> {
        self.latest_by_table.lock().unwrap().get(table).copied()
    }

    fn record_committed(&self, pending: &HashMap<String, TableChangeKind>, seq: u64) {
        let mut latest = self.latest_by_table.lock().unwrap();
        for table in pending.keys() {
            latest.insert(table.clone(), seq);
        }
    }
}

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::broadcast;

use crate::{TableChange, TableChangeKind};

#[derive(Debug)]
pub(crate) struct HookState {
    pending: std::sync::Mutex<HashMap<String, TableChangeKind>>,
    tx: broadcast::Sender<TableChange>,
    change_tracker: Arc<ChangeTracker>,
}

impl HookState {
    pub(crate) fn new(
        tx: broadcast::Sender<TableChange>,
        change_tracker: Arc<ChangeTracker>,
    ) -> Self {
        Self {
            pending: std::sync::Mutex::new(HashMap::new()),
            tx,
            change_tracker,
        }
    }

    pub(crate) fn record(&self, table: &str, kind: TableChangeKind) {
        self.pending.lock().unwrap().insert(table.to_string(), kind);
    }

    pub(crate) fn flush(&self) {
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

    pub(crate) fn clear(&self) {
        self.pending.lock().unwrap().clear();
    }
}

#[derive(Debug, Default)]
pub(crate) struct ChangeTracker {
    current_seq: AtomicU64,
    latest_by_table: std::sync::Mutex<HashMap<String, u64>>,
}

impl ChangeTracker {
    fn next_seq(&self) -> u64 {
        self.current_seq.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub(crate) fn current_seq(&self) -> u64 {
        self.current_seq.load(Ordering::SeqCst)
    }

    pub(crate) fn latest_table_seq(&self, table: &str) -> Option<u64> {
        self.latest_by_table.lock().unwrap().get(table).copied()
    }

    fn record_committed(&self, pending: &HashMap<String, TableChangeKind>, seq: u64) {
        let mut latest = self.latest_by_table.lock().unwrap();
        for table in pending.keys() {
            latest.insert(table.clone(), seq);
        }
    }
}

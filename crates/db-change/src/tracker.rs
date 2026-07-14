use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::broadcast;

use crate::{TableChange, TableChangeKind};

#[derive(Debug)]
pub(crate) struct HookState {
    pending: std::sync::Mutex<PendingChanges>,
    tx: broadcast::Sender<TableChange>,
    change_tracker: Arc<ChangeTracker>,
}

#[derive(Debug, Default)]
struct PendingChanges {
    tables: HashMap<String, TableChangeKind>,
    savepoints: Vec<Savepoint>,
}

#[derive(Debug)]
struct Savepoint {
    name: String,
    starts_transaction: bool,
    tables: HashMap<String, TableChangeKind>,
}

impl HookState {
    pub(crate) fn new(
        tx: broadcast::Sender<TableChange>,
        change_tracker: Arc<ChangeTracker>,
    ) -> Self {
        Self {
            pending: std::sync::Mutex::new(PendingChanges::default()),
            tx,
            change_tracker,
        }
    }

    pub(crate) fn record(&self, table: &str, kind: TableChangeKind) {
        self.pending
            .lock()
            .unwrap()
            .tables
            .insert(table.to_string(), kind);
    }

    pub(crate) fn savepoint(&self, event: hypr_cloudsync::SavepointEvent) {
        let mut pending = self.pending.lock().unwrap();
        match event {
            hypr_cloudsync::SavepointEvent::Begin {
                name,
                starts_transaction,
            } => {
                let tables = pending.tables.clone();
                pending.savepoints.push(Savepoint {
                    name,
                    starts_transaction,
                    tables,
                });
            }
            hypr_cloudsync::SavepointEvent::RollbackTo(name) => {
                if let Some(index) = find_savepoint(&pending.savepoints, &name) {
                    pending.tables = pending.savepoints[index].tables.clone();
                    pending.savepoints.truncate(index + 1);
                }
            }
            hypr_cloudsync::SavepointEvent::Release {
                name,
                transaction_active,
            } => {
                if let Some(index) = find_savepoint(&pending.savepoints, &name) {
                    if pending.savepoints[index].starts_transaction && transaction_active {
                        return;
                    }
                    pending.savepoints.truncate(index);
                }
            }
        }
    }

    pub(crate) fn flush(&self) {
        let pending = {
            let mut state = self.pending.lock().unwrap();
            state.savepoints.clear();
            std::mem::take(&mut state.tables)
        };
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
        *self.pending.lock().unwrap() = PendingChanges::default();
    }
}

fn find_savepoint(savepoints: &[Savepoint], name: &str) -> Option<usize> {
    savepoints
        .iter()
        .rposition(|savepoint| savepoint.name.eq_ignore_ascii_case(name))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_savepoint_names_use_the_most_recent_frame() {
        let (tx, _) = broadcast::channel(1);
        let state = HookState::new(tx, Arc::new(ChangeTracker::default()));
        let name = "_sqlx_savepoint_1".to_string();

        state.record("test_sync", TableChangeKind::Insert);
        state.savepoint(hypr_cloudsync::SavepointEvent::Begin {
            name: name.clone(),
            starts_transaction: false,
        });
        state.record("test_sync", TableChangeKind::Delete);
        state.savepoint(hypr_cloudsync::SavepointEvent::Begin {
            name: name.clone(),
            starts_transaction: false,
        });
        state.record("other_events", TableChangeKind::Insert);

        state.savepoint(hypr_cloudsync::SavepointEvent::RollbackTo(name.clone()));
        {
            let pending = state.pending.lock().unwrap();
            assert_eq!(pending.tables.len(), 1);
            assert_eq!(pending.tables["test_sync"], TableChangeKind::Delete);
            assert_eq!(pending.savepoints.len(), 2);
        }

        state.savepoint(hypr_cloudsync::SavepointEvent::Release {
            name: name.clone(),
            transaction_active: true,
        });
        state.savepoint(hypr_cloudsync::SavepointEvent::RollbackTo(name));
        let pending = state.pending.lock().unwrap();
        assert_eq!(pending.tables.len(), 1);
        assert_eq!(pending.tables["test_sync"], TableChangeKind::Insert);
        assert_eq!(pending.savepoints.len(), 1);
    }
}

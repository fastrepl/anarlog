use std::path::PathBuf;

use hypr_listener_core::{
    ListenerRuntime, SessionDataEvent, SessionErrorEvent, SessionLifecycleEvent,
    SessionProgressEvent,
};
use hypr_listener2_core::{BatchEvent, BatchRuntime};
use tokio::sync::mpsc;

pub(super) enum ListenerEvent {
    Lifecycle(SessionLifecycleEvent),
    Progress(SessionProgressEvent),
    Error(SessionErrorEvent),
    Data(SessionDataEvent),
}

pub(super) struct ListenRuntime {
    vault_base: PathBuf,
    tx: mpsc::UnboundedSender<ListenerEvent>,
}

impl ListenRuntime {
    pub fn new(vault_base: PathBuf, tx: mpsc::UnboundedSender<ListenerEvent>) -> Self {
        Self { vault_base, tx }
    }
}

impl hypr_storage::StorageRuntime for ListenRuntime {
    fn global_base(&self) -> Result<PathBuf, hypr_storage::Error> {
        Ok(self.vault_base.clone())
    }

    fn vault_base(&self) -> Result<PathBuf, hypr_storage::Error> {
        Ok(self.vault_base.clone())
    }
}

impl ListenerRuntime for ListenRuntime {
    fn emit_lifecycle(&self, event: SessionLifecycleEvent) {
        let _ = self.tx.send(ListenerEvent::Lifecycle(event));
    }

    fn emit_progress(&self, event: SessionProgressEvent) {
        let _ = self.tx.send(ListenerEvent::Progress(event));
    }

    fn emit_error(&self, event: SessionErrorEvent) {
        let _ = self.tx.send(ListenerEvent::Error(event));
    }

    fn emit_data(&self, event: SessionDataEvent) {
        let _ = self.tx.send(ListenerEvent::Data(event));
    }
}

pub(super) struct ListenBatchRuntime {
    pub(super) tx: mpsc::UnboundedSender<BatchEvent>,
}

impl BatchRuntime for ListenBatchRuntime {
    fn emit(&self, event: BatchEvent) {
        let _ = self.tx.send(event);
    }
}

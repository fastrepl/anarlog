use std::path::PathBuf;

use hypr_listener_core::{
    ListenerRuntime, SessionDataEvent, SessionErrorEvent, SessionLifecycleEvent,
    SessionProgressEvent,
};
use tokio::sync::mpsc;

pub(super) enum RuntimeEvent {
    Lifecycle(SessionLifecycleEvent),
    Progress(SessionProgressEvent),
    Error(SessionErrorEvent),
    Data(SessionDataEvent),
}

pub(super) struct Runtime {
    vault_base: PathBuf,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
}

impl Runtime {
    pub fn new(vault_base: PathBuf, tx: mpsc::UnboundedSender<RuntimeEvent>) -> Self {
        Self { vault_base, tx }
    }
}

impl hypr_storage::StorageRuntime for Runtime {
    fn global_base(&self) -> Result<PathBuf, hypr_storage::Error> {
        Ok(self.vault_base.clone())
    }

    fn vault_base(&self) -> Result<PathBuf, hypr_storage::Error> {
        Ok(self.vault_base.clone())
    }
}

impl ListenerRuntime for Runtime {
    fn emit_lifecycle(&self, event: SessionLifecycleEvent) {
        let _ = self.tx.send(RuntimeEvent::Lifecycle(event));
    }

    fn emit_progress(&self, event: SessionProgressEvent) {
        let _ = self.tx.send(RuntimeEvent::Progress(event));
    }

    fn emit_error(&self, event: SessionErrorEvent) {
        let _ = self.tx.send(RuntimeEvent::Error(event));
    }

    fn emit_data(&self, event: SessionDataEvent) {
        let _ = self.tx.send(RuntimeEvent::Data(event));
    }
}

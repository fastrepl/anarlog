use crate::events::*;

pub trait ListenerRuntime: hypr_storage::StorageRuntime {
    fn emit_lifecycle(&self, event: SessionLifecycleEvent);
    fn emit_progress(&self, event: SessionProgressEvent);
    fn emit_error(&self, event: SessionErrorEvent);
    fn emit_data(&self, event: SessionDataEvent);
}

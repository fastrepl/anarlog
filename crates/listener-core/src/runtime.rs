use crate::events::*;

pub trait ListenerRuntime: anlg_storage::StorageRuntime {
    fn emit_lifecycle(&self, event: SessionLifecycleEvent);
    /// A root lifecycle event of a session started for one capture (`RootMsg::StartCapture`), so
    /// a late event of an earlier recording cannot act on a newer recording of the same session.
    fn emit_capture_lifecycle(&self, event: SessionLifecycleEvent, capture_id: Option<&str>) {
        let _ = capture_id;
        self.emit_lifecycle(event);
    }
    /// Whether this capture may still start recording (`RootMsg::StartCapture`). `false` once it
    /// was stopped or ended before its start reached the root, so that start is refused.
    fn capture_is_current(&self, capture_id: &str) -> bool {
        let _ = capture_id;
        true
    }
    fn emit_progress(&self, event: SessionProgressEvent);
    fn emit_error(&self, event: SessionErrorEvent);
    fn emit_data(&self, event: SessionDataEvent);
}

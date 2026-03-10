use hypr_listener2_core::{BatchEvent, BatchRuntime};
use tokio::sync::mpsc;

pub(super) struct BatchEventRuntime {
    pub(super) tx: mpsc::UnboundedSender<BatchEvent>,
}

impl BatchRuntime for BatchEventRuntime {
    fn emit(&self, event: BatchEvent) {
        let _ = self.tx.send(event);
    }
}

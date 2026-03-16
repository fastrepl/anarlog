use hypr_listener2_core::{BatchEvent, BatchRuntime};
use tokio::sync::mpsc;

pub struct ChannelBatchRuntime {
    pub tx: mpsc::UnboundedSender<BatchEvent>,
}

impl BatchRuntime for ChannelBatchRuntime {
    fn emit(&self, event: BatchEvent) {
        let _ = self.tx.send(event);
    }
}

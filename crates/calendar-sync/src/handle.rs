use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::{error::RequestSyncError, runtime::SyncStatus};

#[derive(Clone)]
pub struct CalendarSyncHandle {
    tx: mpsc::UnboundedSender<()>,
    status: Arc<Mutex<SyncStatus>>,
}

impl CalendarSyncHandle {
    pub(crate) fn new(tx: mpsc::UnboundedSender<()>, status: Arc<Mutex<SyncStatus>>) -> Self {
        Self { tx, status }
    }

    pub fn request_sync(&self) -> Result<(), RequestSyncError> {
        tracing::info!("calendar sync requested");
        if let Err(error) = self.tx.send(()) {
            tracing::error!(?error, "calendar sync worker is not accepting requests");
            return Err(RequestSyncError);
        }

        Ok(())
    }

    pub fn status(&self) -> SyncStatus {
        *self.status.lock().unwrap()
    }
}

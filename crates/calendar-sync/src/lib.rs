use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;

mod plan;
mod runtime;
mod source;
mod store;
mod types;
mod worker;

pub use plan::{CalendarOp, CalendarPlan, EventOp, EventPlan, plan_calendars, plan_events};
pub use runtime::{CalendarSyncRuntime, CalendarSyncWorkerEvent, SyncStatus};
pub use source::{BoxError, CalendarSyncSource, IncomingSnapshot, SyncOutcome};
pub use store::CalendarSyncStore;
pub use types::{
    CalendarKey, CalendarPayload, ConnectionKey, EventPayload, IncomingCalendar, IncomingEvent,
    IncomingParticipant, PersistedCalendar, PersistedEvent, SyncRange,
};
use worker::SyncWorker;

#[derive(Clone)]
pub struct Config {
    pub interval: Duration,
    pub sync_timeout: Duration,
}

impl Config {
    pub fn every(interval: Duration) -> Self {
        assert!(
            !interval.is_zero(),
            "calendar sync interval must be greater than zero"
        );

        Self {
            interval,
            sync_timeout: Duration::from_secs(30),
        }
    }

    pub fn every_minute() -> Self {
        Self::every(Duration::from_secs(60))
    }
}

#[derive(Clone)]
pub struct CalendarSyncHandle {
    tx: mpsc::UnboundedSender<()>,
    status: Arc<Mutex<SyncStatus>>,
}

#[derive(Debug, Clone, Copy)]
pub struct RequestSyncError;

impl std::fmt::Display for RequestSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("calendar sync worker is not accepting requests")
    }
}

impl std::error::Error for RequestSyncError {}

impl CalendarSyncHandle {
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

pub fn start<S, T, R>(source: S, store: Arc<T>, runtime: R, config: Config) -> CalendarSyncHandle
where
    S: CalendarSyncSource,
    T: CalendarSyncStore,
    R: CalendarSyncRuntime,
{
    let source = Arc::new(source);
    let runtime = Arc::new(runtime);
    let status = Arc::new(Mutex::new(SyncStatus::Idle));
    let (tx, rx) = mpsc::unbounded_channel();
    let worker = SyncWorker::new(
        source,
        store,
        runtime,
        status.clone(),
        rx,
        config.interval,
        config.sync_timeout,
    );

    std::thread::Builder::new()
        .name("calendar-sync-worker".to_string())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to create calendar sync runtime");
                runtime.block_on(worker.run());
            }));

            if let Err(payload) = result {
                let panic_message = if let Some(message) = payload.downcast_ref::<&'static str>() {
                    (*message).to_string()
                } else if let Some(message) = payload.downcast_ref::<String>() {
                    message.clone()
                } else {
                    "unknown panic".to_string()
                };
                tracing::error!(panic = %panic_message, "calendar sync worker thread panicked");
            }
        })
        .expect("failed to spawn calendar sync worker thread");

    CalendarSyncHandle { tx, status }
}

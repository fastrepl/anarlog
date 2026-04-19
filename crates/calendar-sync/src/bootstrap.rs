use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::config::Config;
use crate::handle::CalendarSyncHandle;
use crate::runtime::{CalendarSyncRuntime, SyncStatus};
use crate::source::CalendarSyncSource;
use crate::store::CalendarSyncStore;
use crate::worker::SyncWorker;

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
                tracing::error!(
                    panic = %panic_message(&payload),
                    "calendar sync worker thread panicked"
                );
            }
        })
        .expect("failed to spawn calendar sync worker thread");

    CalendarSyncHandle::new(tx, status)
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic".to_string()
    }
}

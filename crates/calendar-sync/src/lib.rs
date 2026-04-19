mod runtime;
mod source;
mod worker;

pub use runtime::{CalendarSyncRuntime, CalendarSyncWorkerEvent, SyncReason, SyncStatus};
pub use source::{BoxError, CalendarSyncSource, SyncOutcome};

use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;

use apalis::prelude::*;
use apalis_cron::CronStream;
use chrono::Duration;
use cron::Schedule;

use worker::WorkerState;

#[derive(Clone)]
pub struct Config {
    pub schedule: Schedule,
    pub sync_timeout: Duration,
}

impl Config {
    pub fn every_minute() -> Self {
        Self {
            schedule: Schedule::from_str("0 * * * * *")
                .expect("calendar sync schedule must be valid"),
            sync_timeout: Duration::seconds(30),
        }
    }
}

#[derive(Clone)]
pub struct CalendarSyncHandle {
    state: WorkerState,
}

impl CalendarSyncHandle {
    pub fn request_sync(&self, reason: SyncReason) -> SyncStatus {
        self.state.request_sync(reason)
    }

    pub fn status(&self) -> SyncStatus {
        self.state.status()
    }
}

pub type BoxedSyncTask = Pin<Box<dyn Future<Output = ()> + Send>>;

pub fn start(
    source: impl CalendarSyncSource,
    runtime: impl CalendarSyncRuntime,
    config: Config,
    spawn: impl Fn(BoxedSyncTask) + Send + Sync + 'static,
) -> CalendarSyncHandle {
    let state = WorkerState::new(
        source,
        runtime,
        config
            .sync_timeout
            .to_std()
            .expect("calendar sync timeout must be positive"),
    );
    let handle = CalendarSyncHandle {
        state: state.clone(),
    };

    let cron_state = state;
    let schedule = config.schedule;
    spawn(Box::pin(async move {
        let worker = WorkerBuilder::new("calendar-sync-worker")
            .backend(CronStream::new(schedule))
            .data(cron_state)
            .build(worker::enqueue_interval_tick);

        if let Err(error) = worker.run().await {
            tracing::error!("calendar sync cron worker exited: {error}");
        }
    }));

    handle.request_sync(SyncReason::Startup);
    handle
}

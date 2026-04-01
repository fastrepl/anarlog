mod error;
mod worker;

pub mod runtime;
pub mod source;

pub use error::Error;
pub use runtime::{NotificationWorkerEvent, NotificationWorkerRuntime};
pub use source::EventSource;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Duration;

use worker::WorkerState;

pub async fn run(
    source: impl EventSource,
    runtime: impl NotificationWorkerRuntime,
    lookahead: Duration,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::str::FromStr;

    use apalis::prelude::*;
    use apalis_cron::CronStream;
    use cron::Schedule;

    let state = WorkerState {
        source: Arc::new(source),
        runtime: Arc::new(runtime),
        lookahead,
        notified: Arc::new(Mutex::new(HashMap::new())),
    };

    let schedule = Schedule::from_str("0 * * * * *")?;
    let worker = WorkerBuilder::new("notification-worker")
        .backend(CronStream::new(schedule))
        .data(state)
        .build(worker::check_upcoming);

    worker.run().await?;

    Ok(())
}

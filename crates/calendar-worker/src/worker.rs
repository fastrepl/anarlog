use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use apalis::prelude::Data;
use apalis_cron::Tick;
use chrono::{DateTime, Duration, Utc};

use crate::runtime::{NotificationWorkerEvent, NotificationWorkerRuntime};
use crate::source::EventSource;

const DEDUP_TTL_MINUTES: i64 = 30;

// Emit EventStarted when a meeting is within this window around its start time.
// Wide enough to survive a full cron interval on either side.
const START_WINDOW_BEFORE_SECS: i64 = 60;
const START_WINDOW_AFTER_SECS: i64 = 120;

#[derive(Clone)]
pub struct WorkerState {
    pub source: Arc<dyn EventSource>,
    pub runtime: Arc<dyn NotificationWorkerRuntime>,
    pub lookahead: Duration,
    pub notified: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
}

pub async fn do_check(state: &WorkerState) {
    let events = match state.source.upcoming_events(state.lookahead).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("calendar-worker: fetch failed: {e}");
            return;
        }
    };

    let now = Utc::now();

    {
        let mut notified = state.notified.lock().unwrap();
        notified
            .retain(|_, ts| now.signed_duration_since(*ts) < Duration::minutes(DEDUP_TTL_MINUTES));
    }

    for event in events {
        let secs = event.started_at.signed_duration_since(now).num_seconds();

        if secs > START_WINDOW_BEFORE_SECS || secs < -START_WINDOW_AFTER_SECS {
            continue;
        }

        let already = state.notified.lock().unwrap().contains_key(&event.event_id);
        if already {
            continue;
        }

        tracing::info!(
            event_id = %event.event_id,
            title = %event.title,
            secs_until = secs,
            "emitting event started"
        );

        state.runtime.emit(NotificationWorkerEvent::EventStarted {
            event_id: event.event_id.clone(),
            title: event.title,
            started_at: event.started_at.to_rfc3339(),
            participants: event.participants,
        });

        state.notified.lock().unwrap().insert(event.event_id, now);
    }
}

pub async fn check_events(
    _tick: Tick<Utc>,
    ctx: Data<WorkerState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    do_check(&ctx).await;
    Ok(())
}

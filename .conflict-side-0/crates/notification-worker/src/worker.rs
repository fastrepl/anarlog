use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use apalis::prelude::Data;
use apalis_cron::Tick;
use chrono::{Duration, Utc};

use crate::runtime::{NotificationWorkerEvent, NotificationWorkerRuntime};
use crate::source::EventSource;

const DEDUP_TTL_MINUTES: i64 = 30;

#[derive(Clone)]
pub struct WorkerState {
    pub source: Arc<dyn EventSource>,
    pub runtime: Arc<dyn NotificationWorkerRuntime>,
    pub lookahead: Duration,
    pub notified: Arc<Mutex<HashMap<String, chrono::DateTime<Utc>>>>,
}

pub async fn check_upcoming(
    _tick: Tick<Utc>,
    ctx: Data<WorkerState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let events = ctx.source.upcoming_events(ctx.lookahead).await?;

    let now = Utc::now();

    {
        let mut notified = ctx.notified.lock().unwrap();
        notified
            .retain(|_, ts| now.signed_duration_since(*ts) < Duration::minutes(DEDUP_TTL_MINUTES));
    }

    for event in events {
        let already = {
            let notified = ctx.notified.lock().unwrap();
            notified.contains_key(&event.event_id)
        };

        if already {
            continue;
        }

        let minutes_until = event.started_at.signed_duration_since(now).num_minutes();

        tracing::info!(
            event_id = %event.event_id,
            title = %event.title,
            minutes_until,
            "emitting upcoming event notification"
        );

        ctx.runtime.emit(NotificationWorkerEvent::UpcomingEvent {
            event_id: event.event_id.clone(),
            title: event.title,
            started_at: event.started_at.to_rfc3339(),
            minutes_until,
            participants: event.participants,
        });

        ctx.notified.lock().unwrap().insert(event.event_id, now);
    }

    Ok(())
}

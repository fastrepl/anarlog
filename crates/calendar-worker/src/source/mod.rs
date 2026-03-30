use std::pin::Pin;

use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone)]
pub struct UpcomingEvent {
    pub event_id: String,
    pub title: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub participants: Vec<String>,
}

pub trait EventSource: Send + Sync + 'static {
    fn upcoming_events(
        &self,
        within: Duration,
    ) -> Pin<
        Box<dyn std::future::Future<Output = Result<Vec<UpcomingEvent>, crate::Error>> + Send + '_>,
    >;
}

pub mod fs;

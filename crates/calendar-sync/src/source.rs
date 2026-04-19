use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;

use crate::types::{ConnectionKey, IncomingCalendar, IncomingEvent, SyncRange};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, Copy, Default)]
pub struct SyncOutcome {
    pub data_changed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct IncomingSnapshot {
    pub requested_connections: BTreeSet<ConnectionKey>,
    pub successful_calendar_connections: BTreeSet<ConnectionKey>,
    pub successful_event_connections: BTreeSet<ConnectionKey>,
    pub calendars: Vec<IncomingCalendar>,
    pub events: Vec<IncomingEvent>,
}

pub trait CalendarSyncSource: Send + Sync + 'static {
    fn fetch(
        &self,
        range: SyncRange,
    ) -> Pin<Box<dyn Future<Output = Result<IncomingSnapshot, BoxError>> + Send + '_>>;
}

use std::future::Future;
use std::pin::Pin;

use crate::plan::{CalendarPlan, EventPlan};
use crate::source::BoxError;
use crate::types::{PersistedCalendar, PersistedEvent};

pub trait CalendarSyncStore: Send + Sync + 'static {
    type Calendar: PersistedCalendar;
    type Event: PersistedEvent;

    fn read(
        &self,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<(Vec<Self::Calendar>, Vec<Self::Event>), BoxError>>
                + Send
                + '_,
        >,
    >;

    fn apply<'a>(
        &'a self,
        calendar_plan: CalendarPlan<'a>,
        event_plan: EventPlan<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<bool, BoxError>> + Send + 'a>>;
}

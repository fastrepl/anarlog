use std::panic::AssertUnwindSafe;
use std::sync::OnceLock;
use std::time::Duration;

use backon::{BlockingRetryable, ConstantBuilder};
use block2::RcBlock;
use itertools::Itertools;
use objc2::runtime::Bool;
use objc2::{AllocAnyThread, rc::Retained};
use objc2_event_kit::{
    EKAuthorizationStatus, EKCalendar, EKEntityType, EKEvent, EKEventStore, EKSpan,
};
use objc2_foundation::NSString;
use objc2_foundation::{NSArray, NSDate, NSError};

use crate::error::Error;
use crate::types::{AppleCalendar, AppleEvent};
use crate::types::{CreateEventInput, EventFilter};

use super::contacts::ContactFetcher;
use super::transforms::{transform_calendar, transform_event};

fn retry_backoff() -> ConstantBuilder {
    ConstantBuilder::default()
        .with_delay(Duration::from_millis(100))
        .with_max_times(3)
}

struct SendSyncStore(Retained<EKEventStore>);

// SAFETY: EKEventStore is known to be safe to use across threads.
// See: https://stackoverflow.com/a/21372672 and also: https://developer.apple.com/documentation/eventkit/ekeventstore/enumerateevents(matching:using:)
// We enforce a single shared instance via OnceLock to prevent concurrent creation.
unsafe impl Send for SendSyncStore {}
unsafe impl Sync for SendSyncStore {}

static EVENT_STORE: OnceLock<SendSyncStore> = OnceLock::new();

pub(crate) fn shared_event_store() -> &'static EKEventStore {
    &EVENT_STORE
        .get_or_init(|| SendSyncStore(unsafe { EKEventStore::new() }))
        .0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarAuthStatus {
    NotDetermined,
    Authorized,
    Denied,
}

pub struct Handle {
    contact_fetcher: Option<Box<dyn ContactFetcher>>,
}

impl Handle {
    pub fn new() -> Self {
        Self {
            contact_fetcher: None,
        }
    }

    pub fn with_contact_fetcher(contact_fetcher: Box<dyn ContactFetcher>) -> Self {
        Self {
            contact_fetcher: Some(contact_fetcher),
        }
    }

    pub fn authorization_status() -> CalendarAuthStatus {
        let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
        match status {
            EKAuthorizationStatus::NotDetermined => CalendarAuthStatus::NotDetermined,
            EKAuthorizationStatus::FullAccess => CalendarAuthStatus::Authorized,
            _ => CalendarAuthStatus::Denied,
        }
    }

    pub fn request_full_access() -> bool {
        let event_store = shared_event_store();
        let (tx, rx) = std::sync::mpsc::channel();

        let block = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
            let _ = tx.send(granted.as_bool());
        });

        unsafe {
            let ptr = &*block as *const block2::Block<_> as *mut block2::Block<_>;
            event_store.requestFullAccessToEventsWithCompletion(ptr);
        }

        rx.recv_timeout(Duration::from_secs(60)).unwrap_or(false)
    }
}

impl Handle {
    fn has_calendar_access() -> bool {
        let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
        matches!(status, EKAuthorizationStatus::FullAccess)
    }

    fn fetch_events(
        event_store: &EKEventStore,
        filter: &EventFilter,
    ) -> Result<Retained<NSArray<EKEvent>>, Error> {
        let calendars: Retained<NSArray<EKCalendar>> =
            Self::get_calendars_with_exception_handling(event_store)?
                .into_iter()
                .filter(|c| {
                    let id = unsafe { c.calendarIdentifier() }.to_string();
                    filter.calendar_tracking_id.eq(&id)
                })
                .collect();

        if calendars.is_empty() {
            return Err(Error::CalendarNotFound);
        }

        if filter.from > filter.to {
            return Err(Error::InvalidDateRange);
        }

        let (start_date, end_date) = [filter.from, filter.to]
            .iter()
            .sorted_by(|a, b| a.cmp(b))
            .map(|v| NSDate::initWithTimeIntervalSince1970(NSDate::alloc(), v.timestamp() as f64))
            .collect_tuple()
            .ok_or_else(|| Error::InvalidDateRange)?;

        let event_store = AssertUnwindSafe(event_store);
        let calendars = AssertUnwindSafe(calendars);
        let start_date = AssertUnwindSafe(start_date);
        let end_date = AssertUnwindSafe(end_date);

        let result = objc2::exception::catch(|| unsafe {
            let predicate = event_store.predicateForEventsWithStartDate_endDate_calendars(
                &start_date,
                &end_date,
                Some(&calendars),
            );
            event_store.eventsMatchingPredicate(&predicate)
        });

        result.map_err(|_| Error::XpcConnectionFailed)
    }

    fn get_calendars_with_exception_handling(
        event_store: &EKEventStore,
    ) -> Result<Retained<NSArray<EKCalendar>>, Error> {
        let event_store = AssertUnwindSafe(event_store);
        objc2::exception::catch(|| unsafe { event_store.calendars() })
            .map_err(|_| Error::XpcConnectionFailed)
    }

    pub fn list_calendars(&self) -> Result<Vec<AppleCalendar>, Error> {
        if !Self::has_calendar_access() {
            return Err(Error::CalendarAccessDenied);
        }

        let fetch = || {
            let event_store = shared_event_store();
            let calendars = Self::get_calendars_with_exception_handling(event_store)?;
            let list = calendars
                .iter()
                .map(|calendar| transform_calendar(&calendar))
                .sorted_by(|a, b| a.title.cmp(&b.title))
                .collect();
            Ok(list)
        };

        fetch
            .retry(retry_backoff())
            .when(|e| matches!(e, Error::XpcConnectionFailed))
            .call()
    }

    pub fn list_events(&self, filter: EventFilter) -> Result<Vec<AppleEvent>, Error> {
        if !Self::has_calendar_access() {
            return Err(Error::CalendarAccessDenied);
        }

        let contact_fetcher = self.contact_fetcher.as_deref();

        let fetch = || {
            let event_store = shared_event_store();
            let events_array = Self::fetch_events(event_store, &filter)?;

            let events: Result<Vec<_>, _> = events_array
                .iter()
                .filter_map(|event| {
                    let calendar = unsafe { event.calendar() }?;
                    let calendar_id = unsafe { calendar.calendarIdentifier() };

                    if !filter.calendar_tracking_id.eq(&calendar_id.to_string()) {
                        return None;
                    }

                    Some(transform_event(&event, contact_fetcher))
                })
                .collect();

            let mut events = events?;
            events.sort_by(|a, b| a.start_date.cmp(&b.start_date));
            Ok(events)
        };

        fetch
            .retry(retry_backoff())
            .when(|e| matches!(e, Error::XpcConnectionFailed))
            .call()
    }

    pub fn create_event(&self, input: CreateEventInput) -> Result<String, Error> {
        if !Self::has_calendar_access() {
            return Err(Error::CalendarAccessDenied);
        }

        let create = || {
            let event_store = shared_event_store();

            let calendar = Self::get_calendars_with_exception_handling(event_store)?
                .into_iter()
                .find(|c| {
                    let id = unsafe { c.calendarIdentifier() }.to_string();
                    input.calendar_id.eq(&id)
                })
                .ok_or(Error::CalendarNotFound)?;

            let event = unsafe { EKEvent::eventWithEventStore(event_store) };

            unsafe {
                event.setTitle(Some(&NSString::from_str(&input.title)));

                let start_date = NSDate::initWithTimeIntervalSince1970(
                    NSDate::alloc(),
                    input.start_date.timestamp() as f64,
                );
                event.setStartDate(Some(&start_date));

                let end_date = NSDate::initWithTimeIntervalSince1970(
                    NSDate::alloc(),
                    input.end_date.timestamp() as f64,
                );
                event.setEndDate(Some(&end_date));

                event.setCalendar(Some(&calendar));

                if let Some(is_all_day) = input.is_all_day {
                    event.setAllDay(is_all_day);
                }

                if let Some(ref location) = input.location {
                    event.setLocation(Some(&NSString::from_str(location)));
                }

                if let Some(ref notes) = input.notes {
                    event.setNotes(Some(&NSString::from_str(notes)));
                }
            }

            let event_store = AssertUnwindSafe(event_store);
            let event = AssertUnwindSafe(&event);

            let result = objc2::exception::catch(|| unsafe {
                event_store.saveEvent_span_commit_error(&event, EKSpan::ThisEvent, true)
            });

            match result {
                Ok(Ok(())) => {
                    let event_id = unsafe { event.eventIdentifier() }
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    Ok(event_id)
                }
                Ok(Err(ns_error)) => {
                    let error_msg = ns_error.localizedDescription().to_string();
                    Err(Error::ObjectiveCException(error_msg))
                }
                Err(_) => Err(Error::XpcConnectionFailed),
            }
        };

        create
            .retry(retry_backoff())
            .when(|e| matches!(e, Error::XpcConnectionFailed))
            .call()
    }
}

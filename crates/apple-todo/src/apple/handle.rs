use std::panic::AssertUnwindSafe;
use std::sync::OnceLock;
use std::time::Duration;

use backon::{BlockingRetryable, ConstantBuilder};
use block2::RcBlock;
use chrono::{Datelike, Timelike};
use objc2::runtime::Bool;
use objc2::{AllocAnyThread, Message, rc::Retained};
use objc2_event_kit::{
    EKAuthorizationStatus, EKCalendar, EKCalendarItem, EKEntityType, EKEventStore, EKReminder,
};
use objc2_foundation::{
    NSArray, NSCalendar, NSCalendarIdentifierGregorian, NSDate, NSDateComponents, NSError,
    NSString, NSTimeZone, NSURL,
};

use crate::error::Error;
use crate::types::{
    CreateReminderInput, DateComponents, ReadPathResult, ReminderFilter, ReminderFilterKind,
    ReminderIdentifierInput, ReminderList,
};

use super::transforms::{transform_reminder, transform_reminder_list};

fn retry_backoff() -> ConstantBuilder {
    ConstantBuilder::default()
        .with_delay(Duration::from_millis(100))
        .with_max_times(3)
}

struct SendSyncStore(Retained<EKEventStore>);

// SAFETY: EKEventStore is known to be safe to use across threads.
// See: https://stackoverflow.com/a/21372672
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
pub enum ReminderAuthStatus {
    NotDetermined,
    Authorized,
    Denied,
}

pub struct Handle;

impl Default for Handle {
    fn default() -> Self {
        Self::new()
    }
}

impl Handle {
    pub fn new() -> Self {
        Self
    }

    pub fn authorization_status() -> ReminderAuthStatus {
        let status =
            unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Reminder) };
        match status {
            EKAuthorizationStatus::NotDetermined => ReminderAuthStatus::NotDetermined,
            EKAuthorizationStatus::FullAccess => ReminderAuthStatus::Authorized,
            _ => ReminderAuthStatus::Denied,
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
            event_store.requestFullAccessToRemindersWithCompletion(ptr);
        }

        rx.recv_timeout(Duration::from_secs(60)).unwrap_or(false)
    }
}

impl Handle {
    pub fn read_path(&self, path: &str) -> Result<ReadPathResult, Error> {
        match AppleReadPath::parse(path)? {
            AppleReadPath::Lists => self.list_reminder_lists().map(ReadPathResult::Lists),
            AppleReadPath::Reminders { list_id, kind } => self
                .fetch_reminders(ReminderFilter {
                    kind,
                    list_ids: Some(vec![list_id]),
                })
                .map(ReadPathResult::Reminders),
        }
    }

    fn has_reminder_access() -> bool {
        let status =
            unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Reminder) };
        matches!(status, EKAuthorizationStatus::FullAccess)
    }

    fn get_reminder_calendars(
        event_store: &EKEventStore,
    ) -> Result<Retained<NSArray<EKCalendar>>, Error> {
        let event_store = AssertUnwindSafe(event_store);
        objc2::exception::catch(|| unsafe {
            event_store.calendarsForEntityType(EKEntityType::Reminder)
        })
        .map_err(|_| Error::XpcConnectionFailed)
    }

    fn resolve_calendars(
        event_store: &EKEventStore,
        list_ids: &Option<Vec<String>>,
    ) -> Result<Option<Retained<NSArray<EKCalendar>>>, Error> {
        match list_ids {
            Some(ids) => {
                let all_calendars = Self::get_reminder_calendars(event_store)?;
                let filtered: Retained<NSArray<EKCalendar>> = all_calendars
                    .iter()
                    .filter(|c| {
                        let id = unsafe { c.calendarIdentifier() }.to_string();
                        ids.contains(&id)
                    })
                    .collect();
                if filtered.is_empty() {
                    return Err(Error::ReminderListNotFound);
                }
                Ok(Some(filtered))
            }
            None => Ok(None),
        }
    }

    fn fetch_reminders_with_predicate(
        event_store: &EKEventStore,
        filter: &ReminderFilter,
    ) -> Result<Vec<Retained<EKReminder>>, Error> {
        let calendars = Self::resolve_calendars(event_store, &filter.list_ids)?;
        let calendars_ref = calendars.as_deref();

        let event_store = AssertUnwindSafe(event_store);
        let calendars_ref = AssertUnwindSafe(calendars_ref);

        let predicate = objc2::exception::catch(|| unsafe {
            match &filter.kind {
                ReminderFilterKind::All => {
                    event_store.predicateForRemindersInCalendars(*calendars_ref)
                }
                ReminderFilterKind::Incomplete { from, to } => {
                    let start = from.map(|d| {
                        NSDate::initWithTimeIntervalSince1970(NSDate::alloc(), d.timestamp() as f64)
                    });
                    let end = to.map(|d| {
                        NSDate::initWithTimeIntervalSince1970(NSDate::alloc(), d.timestamp() as f64)
                    });
                    event_store.predicateForIncompleteRemindersWithDueDateStarting_ending_calendars(
                        start.as_deref(),
                        end.as_deref(),
                        *calendars_ref,
                    )
                }
                ReminderFilterKind::Completed { from, to } => {
                    let start = from.map(|d| {
                        NSDate::initWithTimeIntervalSince1970(NSDate::alloc(), d.timestamp() as f64)
                    });
                    let end = to.map(|d| {
                        NSDate::initWithTimeIntervalSince1970(NSDate::alloc(), d.timestamp() as f64)
                    });
                    event_store
                        .predicateForCompletedRemindersWithCompletionDateStarting_ending_calendars(
                            start.as_deref(),
                            end.as_deref(),
                            *calendars_ref,
                        )
                }
            }
        })
        .map_err(|_| Error::XpcConnectionFailed)?;

        let (tx, rx) = std::sync::mpsc::channel();

        let block = RcBlock::new(move |reminders: *mut NSArray<EKReminder>| {
            let result = if reminders.is_null() {
                Vec::new()
            } else {
                let arr = unsafe { &*reminders };
                arr.iter().map(|r| Message::retain(&*r)).collect()
            };
            let _ = tx.send(result);
        });

        unsafe {
            event_store.fetchRemindersMatchingPredicate_completion(&predicate, &block);
        }

        rx.recv_timeout(Duration::from_secs(30))
            .map_err(|_| Error::FetchTimeout)
    }

    pub fn list_reminder_lists(&self) -> Result<Vec<ReminderList>, Error> {
        if !Self::has_reminder_access() {
            return Err(Error::ReminderAccessDenied);
        }

        let fetch = || {
            let event_store = shared_event_store();
            let calendars = Self::get_reminder_calendars(event_store)?;
            let default_calendar = unsafe { event_store.defaultCalendarForNewReminders() };
            let default_id =
                default_calendar.map(|c| unsafe { c.calendarIdentifier() }.to_string());

            let mut list: Vec<ReminderList> = calendars
                .iter()
                .map(|calendar| {
                    let id = unsafe { calendar.calendarIdentifier() }.to_string();
                    let is_default = default_id.as_deref() == Some(id.as_str());
                    transform_reminder_list(&calendar, is_default)
                })
                .collect();
            list.sort_by(|a, b| a.title.cmp(&b.title));
            Ok(list)
        };

        fetch
            .retry(retry_backoff())
            .when(|e| matches!(e, Error::XpcConnectionFailed))
            .call()
    }

    pub fn fetch_reminders(
        &self,
        filter: ReminderFilter,
    ) -> Result<Vec<crate::types::Reminder>, Error> {
        if !Self::has_reminder_access() {
            return Err(Error::ReminderAccessDenied);
        }

        let fetch = || {
            let event_store = shared_event_store();
            let reminders = Self::fetch_reminders_with_predicate(event_store, &filter)?;

            let mut result = Vec::new();
            for reminder in &reminders {
                match transform_reminder(reminder) {
                    Ok(r) => result.push(r),
                    Err(e) => {
                        tracing::warn!("failed to transform reminder: {e}");
                    }
                }
            }
            Ok(result)
        };

        fetch
            .retry(retry_backoff())
            .when(|e| matches!(e, Error::XpcConnectionFailed))
            .call()
    }

    pub fn create_reminder(&self, input: CreateReminderInput) -> Result<String, Error> {
        if !Self::has_reminder_access() {
            return Err(Error::ReminderAccessDenied);
        }

        let create = || {
            let event_store = shared_event_store();

            let reminder = unsafe { EKReminder::reminderWithEventStore(event_store) };

            let calendar = Self::resolve_writable_calendar(event_store, input.list_id.as_deref())?;

            unsafe {
                reminder.setTitle(Some(&NSString::from_str(&input.title)));
                reminder.setCalendar(Some(&calendar));

                if let Some(ref notes) = input.notes {
                    reminder.setNotes(Some(&NSString::from_str(notes)));
                }

                if let Some(ref url) = input.url {
                    let ns_url =
                        NSURL::URLWithString(&NSString::from_str(url)).ok_or_else(|| {
                            Error::InvalidDateComponents("invalid reminder url".into())
                        })?;
                    reminder.setURL(Some(&ns_url));
                }

                if let Some(ref priority) = input.priority {
                    let p = priority.to_native();
                    reminder.setPriority(p as usize);
                }

                if let Some(ref due_date) = input.due_date {
                    let components = Self::build_ns_date_components(due_date)?;
                    reminder.setDueDateComponents(Some(&components));
                }

                if let Some(ref start_date) = input.start_date {
                    let components = Self::build_ns_date_components(start_date)?;
                    reminder.setStartDateComponents(Some(&components));
                }
            }

            Self::save_reminder(event_store, &reminder)?;
            let id = unsafe { reminder.calendarItemIdentifier() }.to_string();
            Ok(id)
        };

        create
            .retry(retry_backoff())
            .when(|e| matches!(e, Error::XpcConnectionFailed))
            .call()
    }

    pub fn complete_reminder(&self, target: &ReminderIdentifierInput) -> Result<(), Error> {
        if !Self::has_reminder_access() {
            return Err(Error::ReminderAccessDenied);
        }

        let event_store = shared_event_store();
        let reminder = self.find_reminder(event_store, target)?;
        Self::ensure_reminder_is_writable(&reminder)?;

        unsafe {
            reminder.setCompleted(true);
        }

        Self::save_reminder(event_store, &reminder)
    }

    pub fn delete_reminder(&self, target: &ReminderIdentifierInput) -> Result<(), Error> {
        if !Self::has_reminder_access() {
            return Err(Error::ReminderAccessDenied);
        }

        let event_store = shared_event_store();
        let reminder = self.find_reminder(event_store, target)?;
        Self::ensure_reminder_is_writable(&reminder)?;

        let event_store = AssertUnwindSafe(event_store);
        let reminder = AssertUnwindSafe(&reminder);

        let result = objc2::exception::catch(|| unsafe {
            event_store.removeReminder_commit_error(&reminder, true)
        });

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(ns_error)) => {
                let error_msg = ns_error.localizedDescription().to_string();
                Err(Error::ObjectiveCException(error_msg))
            }
            Err(_) => Err(Error::XpcConnectionFailed),
        }
    }

    fn resolve_writable_calendar(
        event_store: &EKEventStore,
        list_id: Option<&str>,
    ) -> Result<Retained<EKCalendar>, Error> {
        let calendar = if let Some(list_id) = list_id {
            let calendars = Self::get_reminder_calendars(event_store)?;
            calendars
                .iter()
                .find(|c| unsafe { c.calendarIdentifier() }.to_string() == list_id)
                .map(|calendar| Message::retain(&*calendar))
                .ok_or(Error::ReminderListNotFound)?
        } else {
            unsafe { event_store.defaultCalendarForNewReminders() }
                .ok_or(Error::ReminderListNotFound)?
        };

        if unsafe { !calendar.allowsContentModifications() } {
            return Err(Error::ReminderListReadOnly);
        }

        Ok(calendar)
    }

    fn build_ns_date_components(
        input: &DateComponents,
    ) -> Result<Retained<NSDateComponents>, Error> {
        let date = input
            .date
            .ok_or_else(|| Error::InvalidDateComponents("date is required".into()))?;

        let calendar = NSCalendar::calendarWithIdentifier(unsafe { NSCalendarIdentifierGregorian })
            .ok_or_else(|| {
                Error::InvalidDateComponents("failed to create Gregorian calendar".into())
            })?;
        let components = NSDateComponents::new();

        components.setCalendar(Some(&calendar));
        components.setYear(date.year() as isize);
        components.setMonth(date.month() as isize);
        components.setDay(date.day() as isize);

        if let Some(time) = input.time {
            components.setHour(time.hour() as isize);
            components.setMinute(time.minute() as isize);
            components.setSecond(time.second() as isize);
        }

        if let Some(ref time_zone) = input.time_zone {
            let time_zone = NSTimeZone::timeZoneWithName(&NSString::from_str(time_zone))
                .ok_or_else(|| {
                    Error::InvalidDateComponents(format!("unknown timezone: {time_zone}"))
                })?;
            components.setTimeZone(Some(&time_zone));
        }

        Ok(components)
    }

    fn save_reminder(event_store: &EKEventStore, reminder: &EKReminder) -> Result<(), Error> {
        let event_store = AssertUnwindSafe(event_store);
        let reminder_ptr = reminder as *const EKReminder as usize;

        let result = objc2::exception::catch(|| unsafe {
            event_store.saveReminder_commit_error(&*(reminder_ptr as *const EKReminder), true)
        });

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(ns_error)) => {
                let error_msg = ns_error.localizedDescription().to_string();
                Err(Error::ObjectiveCException(error_msg))
            }
            Err(_) => Err(Error::XpcConnectionFailed),
        }
    }

    fn ensure_reminder_is_writable(reminder: &EKReminder) -> Result<(), Error> {
        let calendar = unsafe { reminder.calendar() }.ok_or(Error::ReminderNotFound)?;
        if unsafe { !calendar.allowsContentModifications() } {
            return Err(Error::ReminderListReadOnly);
        }
        Ok(())
    }

    fn find_reminder(
        &self,
        event_store: &EKEventStore,
        target: &ReminderIdentifierInput,
    ) -> Result<Retained<EKReminder>, Error> {
        Self::validate_identifier_input(target)?;

        if let Some(ref calendar_item_identifier) = target.calendar_item_identifier {
            if let Some(reminder) = Self::find_reminder_by_calendar_item_identifier(
                event_store,
                calendar_item_identifier,
            )? {
                return Ok(reminder);
            }
        }

        if let Some(ref external_identifier) = target.external_identifier {
            return Self::find_reminder_by_external_identifier(
                event_store,
                external_identifier,
                target.list_id.as_deref(),
            );
        }

        Err(Error::ReminderNotFound)
    }

    fn validate_identifier_input(target: &ReminderIdentifierInput) -> Result<(), Error> {
        let has_calendar_item_identifier = target
            .calendar_item_identifier
            .as_deref()
            .is_some_and(|value| !value.is_empty());
        let has_external_identifier = target
            .external_identifier
            .as_deref()
            .is_some_and(|value| !value.is_empty());

        if has_calendar_item_identifier || has_external_identifier {
            Ok(())
        } else {
            Err(Error::InvalidReminderIdentifier)
        }
    }

    fn find_reminder_by_calendar_item_identifier(
        event_store: &EKEventStore,
        reminder_id: &str,
    ) -> Result<Option<Retained<EKReminder>>, Error> {
        let event_store = AssertUnwindSafe(event_store);
        let result = objc2::exception::catch(|| unsafe {
            event_store.calendarItemWithIdentifier(&NSString::from_str(reminder_id))
        });

        match result {
            Ok(Some(item)) => Ok(item.downcast::<EKReminder>().ok()),
            Ok(None) => Ok(None),
            Err(_) => Err(Error::XpcConnectionFailed),
        }
    }

    fn find_reminder_by_external_identifier(
        event_store: &EKEventStore,
        external_identifier: &str,
        list_id: Option<&str>,
    ) -> Result<Retained<EKReminder>, Error> {
        let event_store = AssertUnwindSafe(event_store);
        let result = objc2::exception::catch(|| unsafe {
            event_store
                .calendarItemsWithExternalIdentifier(&NSString::from_str(external_identifier))
        });

        let items = result.map_err(|_| Error::XpcConnectionFailed)?;
        let reminders = Self::filter_matching_reminders(items, external_identifier, list_id);

        match reminders.len() {
            0 => Err(Error::ReminderNotFound),
            1 => Ok(reminders
                .into_iter()
                .next()
                .expect("single reminder result")),
            _ => Err(Error::AmbiguousReminderIdentifier),
        }
    }

    fn filter_matching_reminders(
        items: Retained<NSArray<EKCalendarItem>>,
        external_identifier: &str,
        list_id: Option<&str>,
    ) -> Vec<Retained<EKReminder>> {
        items
            .iter()
            .filter_map(|item| Message::retain(&*item).downcast::<EKReminder>().ok())
            .filter(|reminder| {
                let reminder_external_identifier = unsafe {
                    reminder
                        .calendarItemExternalIdentifier()
                        .map(|value| value.to_string())
                        .unwrap_or_default()
                };
                if reminder_external_identifier != external_identifier {
                    return false;
                }

                match list_id {
                    Some(list_id) => unsafe { reminder.calendar() }
                        .map(|calendar| {
                            unsafe { calendar.calendarIdentifier() }.to_string() == list_id
                        })
                        .unwrap_or(false),
                    None => true,
                }
            })
            .collect()
    }
}

#[derive(Debug)]
enum AppleReadPath {
    Lists,
    Reminders {
        list_id: String,
        kind: ReminderFilterKind,
    },
}

impl AppleReadPath {
    fn parse(path: &str) -> Result<Self, Error> {
        let segments = path
            .trim_matches('/')
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();

        match segments.as_slice() {
            [] | ["lists"] => Ok(Self::Lists),
            ["lists", list_id] => Ok(Self::Reminders {
                list_id: (*list_id).to_string(),
                kind: ReminderFilterKind::Incomplete {
                    from: None,
                    to: None,
                },
            }),
            ["lists", list_id, "reminders"] => Ok(Self::Reminders {
                list_id: (*list_id).to_string(),
                kind: ReminderFilterKind::Incomplete {
                    from: None,
                    to: None,
                },
            }),
            ["lists", list_id, "all"] => Ok(Self::Reminders {
                list_id: (*list_id).to_string(),
                kind: ReminderFilterKind::All,
            }),
            ["lists", list_id, "reminders", "all"] => Ok(Self::Reminders {
                list_id: (*list_id).to_string(),
                kind: ReminderFilterKind::All,
            }),
            ["lists", list_id, "incomplete"] => Ok(Self::Reminders {
                list_id: (*list_id).to_string(),
                kind: ReminderFilterKind::Incomplete {
                    from: None,
                    to: None,
                },
            }),
            ["lists", list_id, "reminders", "incomplete"] => Ok(Self::Reminders {
                list_id: (*list_id).to_string(),
                kind: ReminderFilterKind::Incomplete {
                    from: None,
                    to: None,
                },
            }),
            ["lists", list_id, "completed"] => Ok(Self::Reminders {
                list_id: (*list_id).to_string(),
                kind: ReminderFilterKind::Completed {
                    from: None,
                    to: None,
                },
            }),
            ["lists", list_id, "reminders", "completed"] => Ok(Self::Reminders {
                list_id: (*list_id).to_string(),
                kind: ReminderFilterKind::Completed {
                    from: None,
                    to: None,
                },
            }),
            _ => Err(Error::InvalidReadPath(path.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveTime};

    use super::*;

    #[test]
    fn identifier_input_requires_at_least_one_identifier() {
        let result = Handle::validate_identifier_input(&ReminderIdentifierInput {
            calendar_item_identifier: None,
            external_identifier: None,
            list_id: Some("list".into()),
        });

        assert!(matches!(result, Err(Error::InvalidReminderIdentifier)));
    }

    #[test]
    fn build_date_components_supports_all_day_floating_dates() {
        let components = Handle::build_ns_date_components(&DateComponents {
            date: Some(NaiveDate::from_ymd_opt(2026, 4, 7).unwrap()),
            time: None,
            time_zone: None,
        })
        .unwrap();

        assert_eq!(components.year(), 2026);
        assert_eq!(components.month(), 4);
        assert_eq!(components.day(), 7);
        assert_eq!(components.timeZone(), None);
    }

    #[test]
    fn build_date_components_supports_timed_dates_with_timezone() {
        let components = Handle::build_ns_date_components(&DateComponents {
            date: Some(NaiveDate::from_ymd_opt(2026, 4, 7).unwrap()),
            time: Some(NaiveTime::from_hms_opt(9, 30, 15).unwrap()),
            time_zone: Some("Asia/Seoul".into()),
        })
        .unwrap();

        assert_eq!(components.hour(), 9);
        assert_eq!(components.minute(), 30);
        assert_eq!(components.second(), 15);
        assert_eq!(
            components.timeZone().unwrap().name().to_string(),
            "Asia/Seoul"
        );
    }

    #[test]
    fn parse_read_path_defaults_to_incomplete_list_items() {
        let parsed = AppleReadPath::parse("lists/test-list").unwrap();

        match parsed {
            AppleReadPath::Reminders { list_id, kind } => {
                assert_eq!(list_id, "test-list");
                assert!(matches!(
                    kind,
                    ReminderFilterKind::Incomplete {
                        from: None,
                        to: None
                    }
                ));
            }
            AppleReadPath::Lists => panic!("expected reminders path"),
        }
    }

    #[test]
    fn parse_read_path_accepts_explicit_reminders_collection() {
        let parsed = AppleReadPath::parse("/lists/test-list/reminders/").unwrap();

        match parsed {
            AppleReadPath::Reminders { list_id, kind } => {
                assert_eq!(list_id, "test-list");
                assert!(matches!(
                    kind,
                    ReminderFilterKind::Incomplete {
                        from: None,
                        to: None
                    }
                ));
            }
            AppleReadPath::Lists => panic!("expected reminders path"),
        }
    }

    #[test]
    fn parse_read_path_accepts_explicit_completed_filter() {
        let parsed = AppleReadPath::parse("lists/test-list/reminders/completed").unwrap();

        match parsed {
            AppleReadPath::Reminders { list_id, kind } => {
                assert_eq!(list_id, "test-list");
                assert!(matches!(
                    kind,
                    ReminderFilterKind::Completed {
                        from: None,
                        to: None
                    }
                ));
            }
            AppleReadPath::Lists => panic!("expected reminders path"),
        }
    }

    #[test]
    fn parse_read_path_rejects_unknown_paths() {
        let error = AppleReadPath::parse("lists/test-list/unknown").unwrap_err();
        assert!(matches!(error, Error::InvalidReadPath(_)));
    }
}

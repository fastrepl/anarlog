use std::panic::AssertUnwindSafe;
use std::sync::OnceLock;
use std::time::Duration;

use backon::{BlockingRetryable, ConstantBuilder};
use block2::RcBlock;
use objc2::runtime::Bool;
use objc2::{AllocAnyThread, Message, rc::Retained};
use objc2_event_kit::{EKAuthorizationStatus, EKCalendar, EKEntityType, EKEventStore, EKReminder};
use objc2_foundation::{NSArray, NSDate, NSError, NSString};

use crate::error::Error;
use crate::types::{CreateReminderInput, ReminderFilter, ReminderFilterKind, ReminderList};

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

            unsafe {
                reminder.setTitle(Some(&NSString::from_str(&input.title)));

                if let Some(ref notes) = input.notes {
                    reminder.setNotes(Some(&NSString::from_str(notes)));
                }

                if let Some(ref priority) = input.priority {
                    let p = priority.to_native();
                    reminder.setPriority(p as usize);
                }

                if let Some(due_date) = input.due_date {
                    let ns_date = NSDate::initWithTimeIntervalSince1970(
                        NSDate::alloc(),
                        due_date.timestamp() as f64,
                    );
                    let calendar = objc2_foundation::NSCalendar::currentCalendar().retain();
                    let components = calendar.componentsInTimeZone_fromDate(
                        &objc2_foundation::NSTimeZone::localTimeZone(),
                        &ns_date,
                    );
                    reminder.setDueDateComponents(Some(&components));
                }

                if let Some(start_date) = input.start_date {
                    let ns_date = NSDate::initWithTimeIntervalSince1970(
                        NSDate::alloc(),
                        start_date.timestamp() as f64,
                    );
                    let calendar = objc2_foundation::NSCalendar::currentCalendar().retain();
                    let components = calendar.componentsInTimeZone_fromDate(
                        &objc2_foundation::NSTimeZone::localTimeZone(),
                        &ns_date,
                    );
                    reminder.setStartDateComponents(Some(&components));
                }

                if let Some(ref list_id) = input.list_id {
                    let calendars = Self::get_reminder_calendars(event_store)?;
                    let calendar = calendars
                        .iter()
                        .find(|c| {
                            let id = c.calendarIdentifier().to_string();
                            id == *list_id
                        })
                        .ok_or(Error::ReminderListNotFound)?;
                    reminder.setCalendar(Some(&calendar));
                } else if let Some(default) = event_store.defaultCalendarForNewReminders() {
                    reminder.setCalendar(Some(&default));
                }
            }

            let event_store = AssertUnwindSafe(event_store);
            let reminder = AssertUnwindSafe(&reminder);

            let result = objc2::exception::catch(|| unsafe {
                event_store.saveReminder_commit_error(&reminder, true)
            });

            match result {
                Ok(Ok(())) => {
                    let id = unsafe { reminder.calendarItemIdentifier() }.to_string();
                    Ok(id)
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

    pub fn complete_reminder(&self, reminder_id: &str) -> Result<(), Error> {
        if !Self::has_reminder_access() {
            return Err(Error::ReminderAccessDenied);
        }

        let event_store = shared_event_store();
        let reminder = self.find_reminder(event_store, reminder_id)?;

        unsafe {
            reminder.setCompleted(true);
        }

        let event_store = AssertUnwindSafe(event_store);
        let reminder = AssertUnwindSafe(&reminder);

        let result = objc2::exception::catch(|| unsafe {
            event_store.saveReminder_commit_error(&reminder, true)
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

    pub fn delete_reminder(&self, reminder_id: &str) -> Result<(), Error> {
        if !Self::has_reminder_access() {
            return Err(Error::ReminderAccessDenied);
        }

        let event_store = shared_event_store();
        let reminder = self.find_reminder(event_store, reminder_id)?;

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

    fn find_reminder(
        &self,
        event_store: &EKEventStore,
        reminder_id: &str,
    ) -> Result<Retained<EKReminder>, Error> {
        let event_store = AssertUnwindSafe(event_store);
        let result = objc2::exception::catch(|| unsafe {
            event_store.calendarItemWithIdentifier(&NSString::from_str(reminder_id))
        });

        match result {
            Ok(Some(item)) => {
                let reminder: Retained<EKReminder> = unsafe { Retained::cast_unchecked(item) };
                Ok(reminder)
            }
            Ok(None) => Err(Error::ReminderNotFound),
            Err(_) => Err(Error::XpcConnectionFailed),
        }
    }
}

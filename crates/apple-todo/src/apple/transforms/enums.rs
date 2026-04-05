use objc2_event_kit::{EKCalendarType, EKSourceType};

use crate::types::{CalendarSourceType, CalendarType};

pub fn transform_calendar_type(t: EKCalendarType) -> CalendarType {
    match t {
        EKCalendarType::Local => CalendarType::Local,
        EKCalendarType::CalDAV => CalendarType::CalDav,
        EKCalendarType::Exchange => CalendarType::Exchange,
        EKCalendarType::Subscription => CalendarType::Subscription,
        EKCalendarType::Birthday => CalendarType::Birthday,
        _ => CalendarType::Local,
    }
}

pub fn transform_source_type(t: EKSourceType) -> CalendarSourceType {
    match t {
        EKSourceType::Local => CalendarSourceType::Local,
        EKSourceType::Exchange => CalendarSourceType::Exchange,
        EKSourceType::CalDAV => CalendarSourceType::CalDav,
        EKSourceType::MobileMe => CalendarSourceType::MobileMe,
        EKSourceType::Subscribed => CalendarSourceType::Subscribed,
        EKSourceType::Birthdays => CalendarSourceType::Birthdays,
        _ => CalendarSourceType::Local,
    }
}

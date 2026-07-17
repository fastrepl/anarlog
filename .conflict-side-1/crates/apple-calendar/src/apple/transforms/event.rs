use objc2::{msg_send, rc::Retained, runtime::Bool};
use objc2_event_kit::{EKAlarm, EKCalendarType, EKEvent, EKStructuredLocation};
use objc2_foundation::{NSArray, NSDate, NSString, NSTimeZone};

use crate::error::Error;
use crate::types::{AppleEvent, CalendarRef, Participant, StructuredLocation};

use super::super::contacts::ContactFetcher;
use super::super::recurrence::{offset_date_time_from, parse_recurrence_info};
use super::alarm::transform_alarm;
use super::enums::{transform_event_availability, transform_event_status};
use super::location::transform_structured_location;
use super::participant::transform_participant;
use super::utils::get_url_string;

pub fn transform_event(
    event: &EKEvent,
    contact_fetcher: Option<&dyn ContactFetcher>,
) -> Result<AppleEvent, Error> {
    let identifiers = extract_event_identifiers(event);
    let calendar_ref = extract_event_calendar_ref(event);
    let basic_info = extract_event_basic_info(event);
    let dates = extract_event_dates(event);
    let status_info = extract_event_status_info(event);
    let flags = extract_event_flags(event);
    let participants = extract_event_participants(event, contact_fetcher);
    let location_info = extract_event_location_info(event);
    let recurrence_info = extract_event_recurrence_info(event, flags.has_recurrence_rules);
    let alarm_info = extract_event_alarm_info(event);
    let birthday_info = extract_event_birthday_info(event, &calendar_ref);

    Ok(AppleEvent {
        event_identifier: identifiers.event_identifier,
        calendar_item_identifier: identifiers.calendar_item_identifier,
        external_identifier: identifiers.external_identifier,
        calendar: calendar_ref,
        title: basic_info.title,
        location: basic_info.location,
        url: basic_info.url,
        notes: basic_info.notes,
        creation_date: basic_info.creation_date,
        last_modified_date: basic_info.last_modified_date,
        time_zone: basic_info.time_zone,
        start_date: dates.start_date,
        end_date: dates.end_date,
        is_all_day: dates.is_all_day,
        availability: status_info.availability,
        status: status_info.status,
        has_alarms: flags.has_alarms,
        has_attendees: flags.has_attendees,
        has_notes: flags.has_notes,
        has_recurrence_rules: flags.has_recurrence_rules,
        organizer: participants.organizer,
        attendees: participants.attendees,
        structured_location: location_info.structured_location,
        recurrence: recurrence_info.recurrence,
        occurrence_date: recurrence_info.occurrence_date,
        is_detached: recurrence_info.is_detached,
        alarms: alarm_info.alarms,
        birthday_contact_identifier: birthday_info.birthday_contact_identifier,
        is_birthday: birthday_info.is_birthday,
    })
}

struct EventIdentifiers {
    event_identifier: String,
    calendar_item_identifier: String,
    external_identifier: String,
}

fn extract_event_identifiers(event: &EKEvent) -> EventIdentifiers {
    EventIdentifiers {
        event_identifier: unsafe { event.eventIdentifier() }
            .map(|s| s.to_string())
            .unwrap_or_default(),
        calendar_item_identifier: unsafe { event.calendarItemIdentifier() }.to_string(),
        external_identifier: unsafe { event.calendarItemExternalIdentifier() }
            .map(|s| s.to_string())
            .unwrap_or_default(),
    }
}

fn extract_event_calendar_ref(event: &EKEvent) -> CalendarRef {
    let calendar = unsafe { event.calendar() }.unwrap();
    CalendarRef {
        id: unsafe { calendar.calendarIdentifier() }.to_string(),
        title: unsafe { calendar.title() }.to_string(),
    }
}

struct EventBasicInfo {
    title: String,
    location: Option<String>,
    url: Option<String>,
    notes: Option<String>,
    creation_date: Option<chrono::DateTime<chrono::Utc>>,
    last_modified_date: Option<chrono::DateTime<chrono::Utc>>,
    time_zone: Option<String>,
}

fn extract_event_basic_info(event: &EKEvent) -> EventBasicInfo {
    EventBasicInfo {
        title: unsafe { event.title() }.to_string(),
        location: unsafe { event.location() }.map(|s| s.to_string()),
        url: get_url_string(event, "URL"),
        notes: unsafe { event.notes() }.map(|s| s.to_string()),
        creation_date: unsafe {
            let date: Option<Retained<NSDate>> = msg_send![event, creationDate];
            date.map(offset_date_time_from)
        },
        last_modified_date: unsafe {
            let date: Option<Retained<NSDate>> = msg_send![event, lastModifiedDate];
            date.map(offset_date_time_from)
        },
        time_zone: unsafe {
            let tz: Option<Retained<NSTimeZone>> = msg_send![event, timeZone];
            tz.map(|t| t.name().to_string())
        },
    }
}

struct EventDates {
    start_date: chrono::DateTime<chrono::Utc>,
    end_date: chrono::DateTime<chrono::Utc>,
    is_all_day: bool,
}

fn extract_event_dates(event: &EKEvent) -> EventDates {
    EventDates {
        start_date: offset_date_time_from(unsafe { event.startDate() }),
        end_date: offset_date_time_from(unsafe { event.endDate() }),
        is_all_day: unsafe { event.isAllDay() },
    }
}

struct EventStatusInfo {
    availability: crate::types::EventAvailability,
    status: crate::types::EventStatus,
}

fn extract_event_status_info(event: &EKEvent) -> EventStatusInfo {
    EventStatusInfo {
        availability: transform_event_availability(unsafe { event.availability() }),
        status: transform_event_status(unsafe { event.status() }),
    }
}

struct EventFlags {
    has_alarms: bool,
    has_attendees: bool,
    has_notes: bool,
    has_recurrence_rules: bool,
}

fn extract_event_flags(event: &EKEvent) -> EventFlags {
    EventFlags {
        has_alarms: unsafe {
            let b: Bool = msg_send![event, hasAlarms];
            b.as_bool()
        },
        has_attendees: unsafe {
            let b: Bool = msg_send![event, hasAttendees];
            b.as_bool()
        },
        has_notes: unsafe {
            let b: Bool = msg_send![event, hasNotes];
            b.as_bool()
        },
        has_recurrence_rules: unsafe {
            let b: Bool = msg_send![event, hasRecurrenceRules];
            b.as_bool()
        },
    }
}

struct EventParticipants {
    organizer: Option<Participant>,
    attendees: Vec<Participant>,
}

fn extract_event_participants(
    event: &EKEvent,
    contact_fetcher: Option<&dyn ContactFetcher>,
) -> EventParticipants {
    EventParticipants {
        organizer: unsafe { event.organizer() }.map(|p| transform_participant(&p, contact_fetcher)),
        attendees: unsafe { event.attendees() }
            .map(|arr| {
                arr.iter()
                    .map(|p| transform_participant(&p, contact_fetcher))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

struct EventLocationInfo {
    structured_location: Option<StructuredLocation>,
}

fn extract_event_location_info(event: &EKEvent) -> EventLocationInfo {
    EventLocationInfo {
        structured_location: unsafe {
            let loc: Option<Retained<EKStructuredLocation>> = msg_send![event, structuredLocation];
            loc.map(|l| transform_structured_location(&l))
        },
    }
}

struct EventRecurrenceInfo {
    recurrence: Option<crate::types::RecurrenceInfo>,
    occurrence_date: Option<chrono::DateTime<chrono::Utc>>,
    is_detached: bool,
}

fn extract_event_recurrence_info(
    event: &EKEvent,
    has_recurrence_rules: bool,
) -> EventRecurrenceInfo {
    EventRecurrenceInfo {
        recurrence: parse_recurrence_info(event, has_recurrence_rules),
        occurrence_date: unsafe { event.occurrenceDate() }.map(offset_date_time_from),
        is_detached: unsafe { event.isDetached() },
    }
}

struct EventAlarmInfo {
    alarms: Vec<crate::types::Alarm>,
}

fn extract_event_alarm_info(event: &EKEvent) -> EventAlarmInfo {
    EventAlarmInfo {
        alarms: unsafe {
            let alarm_arr: Option<Retained<NSArray<EKAlarm>>> = msg_send![event, alarms];
            alarm_arr
                .map(|arr| arr.iter().map(|a| transform_alarm(&a)).collect())
                .unwrap_or_default()
        },
    }
}

struct EventBirthdayInfo {
    birthday_contact_identifier: Option<String>,
    is_birthday: bool,
}

fn extract_event_birthday_info(event: &EKEvent, _calendar_ref: &CalendarRef) -> EventBirthdayInfo {
    let birthday_contact_identifier = unsafe {
        let id: Option<Retained<NSString>> = msg_send![event, birthdayContactIdentifier];
        id.map(|s| s.to_string())
    };

    let is_birthday = birthday_contact_identifier.is_some()
        || unsafe { event.calendar().unwrap().r#type() } == EKCalendarType::Birthday;

    EventBirthdayInfo {
        birthday_contact_identifier,
        is_birthday,
    }
}

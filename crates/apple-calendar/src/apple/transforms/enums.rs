use objc2_event_kit::{
    EKCalendarType, EKEventAvailability, EKEventStatus, EKParticipantRole, EKParticipantStatus,
    EKParticipantType, EKSourceType,
};

use crate::types::{
    CalendarSourceType, CalendarType, EventAvailability, EventStatus, ParticipantRole,
    ParticipantStatus, ParticipantType,
};

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

pub fn transform_event_availability(a: EKEventAvailability) -> EventAvailability {
    match a {
        EKEventAvailability::NotSupported => EventAvailability::NotSupported,
        EKEventAvailability::Busy => EventAvailability::Busy,
        EKEventAvailability::Free => EventAvailability::Free,
        EKEventAvailability::Tentative => EventAvailability::Tentative,
        EKEventAvailability::Unavailable => EventAvailability::Unavailable,
        _ => EventAvailability::NotSupported,
    }
}

pub fn transform_event_status(s: EKEventStatus) -> EventStatus {
    match s {
        EKEventStatus::None => EventStatus::None,
        EKEventStatus::Confirmed => EventStatus::Confirmed,
        EKEventStatus::Tentative => EventStatus::Tentative,
        EKEventStatus::Canceled => EventStatus::Canceled,
        _ => EventStatus::None,
    }
}

pub fn transform_participant_role(r: EKParticipantRole) -> ParticipantRole {
    match r {
        EKParticipantRole::Unknown => ParticipantRole::Unknown,
        EKParticipantRole::Required => ParticipantRole::Required,
        EKParticipantRole::Optional => ParticipantRole::Optional,
        EKParticipantRole::Chair => ParticipantRole::Chair,
        EKParticipantRole::NonParticipant => ParticipantRole::NonParticipant,
        _ => ParticipantRole::Unknown,
    }
}

pub fn transform_participant_status(s: EKParticipantStatus) -> ParticipantStatus {
    match s {
        EKParticipantStatus::Unknown => ParticipantStatus::Unknown,
        EKParticipantStatus::Pending => ParticipantStatus::Pending,
        EKParticipantStatus::Accepted => ParticipantStatus::Accepted,
        EKParticipantStatus::Declined => ParticipantStatus::Declined,
        EKParticipantStatus::Tentative => ParticipantStatus::Tentative,
        EKParticipantStatus::Delegated => ParticipantStatus::Delegated,
        EKParticipantStatus::Completed => ParticipantStatus::Completed,
        EKParticipantStatus::InProcess => ParticipantStatus::InProgress,
        _ => ParticipantStatus::Unknown,
    }
}

pub fn transform_participant_type(t: EKParticipantType) -> ParticipantType {
    match t {
        EKParticipantType::Unknown => ParticipantType::Unknown,
        EKParticipantType::Person => ParticipantType::Person,
        EKParticipantType::Room => ParticipantType::Room,
        EKParticipantType::Resource => ParticipantType::Resource,
        EKParticipantType::Group => ParticipantType::Group,
        _ => ParticipantType::Unknown,
    }
}

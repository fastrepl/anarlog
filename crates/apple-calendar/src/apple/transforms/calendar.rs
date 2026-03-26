use objc2_event_kit::EKCalendar;

use crate::types::{AppleCalendar, CalendarSource, CalendarType};

use super::enums::{transform_calendar_type, transform_source_type};
use super::utils::{
    extract_allowed_entity_types, extract_color_components, extract_supported_availabilities,
};

pub fn transform_calendar(calendar: &EKCalendar) -> AppleCalendar {
    let id = unsafe { calendar.calendarIdentifier() }.to_string();
    let title = unsafe { calendar.title() }.to_string();
    let calendar_type = transform_calendar_type(unsafe { calendar.r#type() });
    let color = unsafe { calendar.CGColor() }.map(|cg_color| extract_color_components(&cg_color));

    let properties = extract_calendar_properties(calendar);

    AppleCalendar {
        id,
        title,
        calendar_type,
        color,
        ..properties
    }
}

pub fn extract_calendar_properties(calendar: &EKCalendar) -> AppleCalendar {
    let allows_content_modifications = unsafe { calendar.allowsContentModifications() };
    let is_immutable = unsafe { calendar.isImmutable() };
    let is_subscribed = unsafe { calendar.isSubscribed() };
    let supported_event_availabilities = extract_supported_availabilities(calendar);
    let allowed_entity_types = extract_allowed_entity_types(calendar);
    let source = extract_calendar_source(calendar);

    AppleCalendar {
        allows_content_modifications,
        is_immutable,
        is_subscribed,
        supported_event_availabilities,
        allowed_entity_types,
        source,
        id: String::new(),
        title: String::new(),
        calendar_type: CalendarType::Local,
        color: None,
    }
}

pub fn extract_calendar_source(calendar: &EKCalendar) -> CalendarSource {
    if let Some(src) = unsafe { calendar.source() } {
        let source_identifier = unsafe { src.sourceIdentifier() }.to_string();
        let source_title = unsafe { src.title() }.to_string();
        let source_type = transform_source_type(unsafe { src.sourceType() });
        CalendarSource {
            identifier: source_identifier,
            title: source_title,
            source_type,
        }
    } else {
        CalendarSource::default()
    }
}

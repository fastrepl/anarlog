use objc2_event_kit::EKCalendar;

use crate::types::{CalendarSource, ReminderList};

use super::enums::{transform_calendar_type, transform_source_type};
use super::utils::{extract_color_components, objc_source_identifier, objc_title};

pub fn transform_reminder_list(calendar: &EKCalendar, is_default: bool) -> ReminderList {
    let id = unsafe { calendar.calendarIdentifier() }.to_string();
    let title = objc_title(calendar);
    let calendar_type = transform_calendar_type(unsafe { calendar.r#type() });
    let color = unsafe { calendar.CGColor() }.map(|cg_color| extract_color_components(&cg_color));
    let allows_content_modifications = unsafe { calendar.allowsContentModifications() };
    let source = extract_source(calendar);

    ReminderList {
        id,
        title,
        calendar_type,
        color,
        allows_content_modifications,
        is_default,
        source,
    }
}

fn extract_source(calendar: &EKCalendar) -> CalendarSource {
    if let Some(src) = unsafe { calendar.source() } {
        let source_identifier = objc_source_identifier(&*src);
        let source_title = objc_title(&*src);
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

use objc2::{msg_send, rc::Retained};
use objc2_event_kit::{EKAlarm, EKReminder};
use objc2_foundation::{NSArray, NSDate, NSDateComponents, NSInteger, NSURL};

use crate::error::{Error, Result};
use crate::types::{DateComponents, Reminder, ReminderListRef, ReminderPriority};

use super::super::recurrence::{offset_date_time_from, parse_recurrence_rules};
use super::alarm::transform_alarm;

pub fn transform_reminder(reminder: &EKReminder) -> Result<Reminder> {
    let calendar_item_identifier = unsafe { reminder.calendarItemIdentifier() }.to_string();

    let external_identifier = unsafe { reminder.calendarItemExternalIdentifier() }
        .map(|s| s.to_string())
        .unwrap_or_default();

    let list = unsafe { reminder.calendar() }
        .map(|cal| ReminderListRef {
            id: unsafe { cal.calendarIdentifier() }.to_string(),
            title: unsafe { cal.title() }.to_string(),
        })
        .ok_or_else(|| Error::TransformError("reminder has no calendar".into()))?;

    let title = unsafe {
        let t: Option<Retained<objc2_foundation::NSString>> = msg_send![reminder, title];
        t.map(|s| s.to_string()).unwrap_or_default()
    };

    let notes = unsafe { reminder.notes() }.map(|s| s.to_string());

    let url = unsafe {
        let url_obj: Option<Retained<NSURL>> = msg_send![reminder, URL];
        url_obj.and_then(|u| u.absoluteString().map(|s| s.to_string()))
    };

    let priority: NSInteger = unsafe { msg_send![reminder, priority] };
    let priority = ReminderPriority::from_native(priority as i64);

    let is_completed = unsafe { reminder.isCompleted() };

    let completion_date = unsafe { reminder.completionDate() }.map(offset_date_time_from);

    let start_date_components =
        unsafe { reminder.startDateComponents() }.map(|c| extract_date_components(&c));

    let due_date_components =
        unsafe { reminder.dueDateComponents() }.map(|c| extract_date_components(&c));

    let creation_date = unsafe {
        let date: Option<Retained<NSDate>> = msg_send![reminder, creationDate];
        date.map(offset_date_time_from)
    };

    let last_modified_date = unsafe {
        let date: Option<Retained<NSDate>> = msg_send![reminder, lastModifiedDate];
        date.map(offset_date_time_from)
    };

    let has_alarms = unsafe { reminder.hasAlarms() };
    let has_recurrence_rules = unsafe { reminder.hasRecurrenceRules() };

    let alarms = unsafe {
        let alarms: Option<Retained<NSArray<EKAlarm>>> = msg_send![reminder, alarms];
        alarms
            .map(|arr| arr.iter().map(|a| transform_alarm(&a)).collect())
            .unwrap_or_default()
    };

    let recurrence_rules = parse_recurrence_rules(reminder);

    Ok(Reminder {
        calendar_item_identifier,
        external_identifier,
        list,
        title,
        notes,
        url,
        priority,
        is_completed,
        completion_date,
        start_date_components,
        due_date_components,
        creation_date,
        last_modified_date,
        has_alarms,
        has_recurrence_rules,
        alarms,
        recurrence_rules,
    })
}

fn extract_date_components(components: &NSDateComponents) -> DateComponents {
    let year = components.year();
    let month = components.month();
    let day = components.day();
    let hour = components.hour();
    let minute = components.minute();

    // NSDateComponents uses NSIntegerMax (isize::MAX) for undefined components
    let undefined = isize::MAX;

    let date = if year != undefined && month != undefined && day != undefined {
        chrono::NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32)
    } else {
        None
    };

    let time = if hour != undefined && minute != undefined {
        chrono::NaiveTime::from_hms_opt(hour as u32, minute as u32, 0)
    } else {
        None
    };

    let time_zone = components.timeZone().map(|tz| tz.name().to_string());

    DateComponents {
        date,
        time,
        time_zone,
    }
}

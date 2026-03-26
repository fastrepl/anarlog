use objc2::{msg_send, rc::Retained};
use objc2_event_kit::{
    EKEvent, EKRecurrenceDayOfWeek, EKRecurrenceEnd, EKRecurrenceFrequency, EKRecurrenceRule,
    EKWeekday,
};
use objc2_foundation::{NSArray, NSDate, NSInteger, NSNumber};

use crate::types::{
    RecurrenceDayOfWeek, RecurrenceEnd, RecurrenceFrequency, RecurrenceInfo, RecurrenceOccurrence,
    RecurrenceRule, Weekday,
};

fn series_id(event: &EKEvent) -> String {
    unsafe {
        event
            .calendarItemExternalIdentifier()
            .map(|s| s.to_string())
            .unwrap_or_else(|| event.calendarItemIdentifier().to_string())
    }
}

pub fn parse_recurrence_info(
    event: &EKEvent,
    has_recurrence_rules: bool,
) -> Option<RecurrenceInfo> {
    // Only return RecurrenceInfo for events that actually have recurrence rules.
    // Single events may have occurrence_date populated but should not be treated
    // as recurring events.
    if !has_recurrence_rules {
        return None;
    }

    let occurrence_date = unsafe { event.occurrenceDate() };

    let occurrence = occurrence_date.map(|date| RecurrenceOccurrence {
        original_start: offset_date_time_from(date),
        is_detached: unsafe { event.isDetached() },
    });

    let rules = parse_recurrence_rules(event);

    Some(RecurrenceInfo {
        series_identifier: series_id(event),
        has_recurrence_rules,
        occurrence,
        rules,
    })
}

fn parse_recurrence_rules(event: &EKEvent) -> Vec<RecurrenceRule> {
    unsafe {
        let rules: Option<Retained<NSArray<EKRecurrenceRule>>> = msg_send![event, recurrenceRules];
        rules
            .map(|arr| arr.iter().filter_map(|r| parse_single_rule(&r)).collect())
            .unwrap_or_default()
    }
}

fn parse_single_rule(rule: &EKRecurrenceRule) -> Option<RecurrenceRule> {
    let frequency = match unsafe { rule.frequency() } {
        EKRecurrenceFrequency::Daily => RecurrenceFrequency::Daily,
        EKRecurrenceFrequency::Weekly => RecurrenceFrequency::Weekly,
        EKRecurrenceFrequency::Monthly => RecurrenceFrequency::Monthly,
        EKRecurrenceFrequency::Yearly => RecurrenceFrequency::Yearly,
        _ => return None,
    };

    let interval = (unsafe { rule.interval() } as u32).max(1);

    let days_of_week = unsafe {
        let dow: Option<Retained<NSArray<EKRecurrenceDayOfWeek>>> = msg_send![rule, daysOfTheWeek];
        dow.map(|arr| {
            arr.iter()
                .map(|d| {
                    let weekday = transform_weekday(d.dayOfTheWeek());
                    let week_number = {
                        let wn = d.weekNumber();
                        if wn == 0 { None } else { Some(wn as i8) }
                    };
                    RecurrenceDayOfWeek {
                        weekday,
                        week_number,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
    };

    let days_of_month = unsafe {
        let dom: Option<Retained<NSArray<NSNumber>>> = msg_send![rule, daysOfTheMonth];
        dom.map(|arr| arr.iter().map(|n| n.intValue() as i8).collect())
            .unwrap_or_default()
    };

    let months_of_year = unsafe {
        let moy: Option<Retained<NSArray<NSNumber>>> = msg_send![rule, monthsOfTheYear];
        moy.map(|arr| arr.iter().map(|n| n.intValue() as u8).collect())
            .unwrap_or_default()
    };

    let weeks_of_year = unsafe {
        let woy: Option<Retained<NSArray<NSNumber>>> = msg_send![rule, weeksOfTheYear];
        woy.map(|arr| arr.iter().map(|n| n.intValue() as i8).collect())
            .unwrap_or_default()
    };

    let days_of_year = unsafe {
        let doy: Option<Retained<NSArray<NSNumber>>> = msg_send![rule, daysOfTheYear];
        doy.map(|arr| arr.iter().map(|n| n.intValue() as i16).collect())
            .unwrap_or_default()
    };

    let set_positions = unsafe {
        let sp: Option<Retained<NSArray<NSNumber>>> = msg_send![rule, setPositions];
        sp.map(|arr| arr.iter().map(|n| n.intValue() as i16).collect())
            .unwrap_or_default()
    };

    let first_day_of_week = unsafe {
        let fdow: NSInteger = msg_send![rule, firstDayOfTheWeek];
        if fdow == 0 {
            None
        } else {
            Some(transform_weekday_from_int(fdow))
        }
    };

    let end = parse_recurrence_end(rule);

    Some(RecurrenceRule {
        frequency,
        interval,
        days_of_week,
        days_of_month,
        months_of_year,
        weeks_of_year,
        days_of_year,
        set_positions,
        first_day_of_week,
        end,
    })
}

fn parse_recurrence_end(rule: &EKRecurrenceRule) -> Option<RecurrenceEnd> {
    unsafe {
        let end: Option<Retained<EKRecurrenceEnd>> = msg_send![rule, recurrenceEnd];
        let end = end?;

        let end_date: Option<Retained<NSDate>> = msg_send![&*end, endDate];
        if let Some(date) = end_date {
            return Some(RecurrenceEnd::Until(offset_date_time_from(date)));
        }

        let occurrence_count: NSInteger = msg_send![&*end, occurrenceCount];
        if occurrence_count > 0 {
            return Some(RecurrenceEnd::Count(occurrence_count as u32));
        }

        None
    }
}

fn transform_weekday(w: EKWeekday) -> Weekday {
    match w {
        EKWeekday::Sunday => Weekday::Sunday,
        EKWeekday::Monday => Weekday::Monday,
        EKWeekday::Tuesday => Weekday::Tuesday,
        EKWeekday::Wednesday => Weekday::Wednesday,
        EKWeekday::Thursday => Weekday::Thursday,
        EKWeekday::Friday => Weekday::Friday,
        EKWeekday::Saturday => Weekday::Saturday,
        _ => Weekday::Sunday,
    }
}

fn transform_weekday_from_int(w: NSInteger) -> Weekday {
    match w {
        1 => Weekday::Sunday,
        2 => Weekday::Monday,
        3 => Weekday::Tuesday,
        4 => Weekday::Wednesday,
        5 => Weekday::Thursday,
        6 => Weekday::Friday,
        7 => Weekday::Saturday,
        _ => Weekday::Sunday,
    }
}

pub fn offset_date_time_from(date: Retained<NSDate>) -> chrono::DateTime<chrono::Utc> {
    let seconds = date.timeIntervalSinceReferenceDate();

    let cocoa_reference: chrono::DateTime<chrono::Utc> =
        chrono::DateTime::from_naive_utc_and_offset(
            chrono::NaiveDateTime::new(
                chrono::NaiveDate::from_ymd_opt(2001, 1, 1).unwrap(),
                chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            ),
            chrono::Utc,
        );

    let unix_timestamp = seconds + cocoa_reference.timestamp() as f64;
    chrono::DateTime::<chrono::Utc>::from_timestamp(unix_timestamp as i64, 0).unwrap()
}

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};

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
fn validate_create_input_rejects_conflicting_completion_state() {
    let completed_at = DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let input = CreateReminderInput {
        title: "Ship API".into(),
        list_id: None,
        notes: None,
        url: None,
        priority: None,
        due_date: None,
        start_date: None,
        alarms: None,
        recurrence_rules: None,
        is_completed: Some(false),
        completion_date: Some(completed_at),
    };

    let error = Handle::validate_create_input(&input).unwrap_err();
    assert!(matches!(error, Error::InvalidReminderInput(_)));
}

#[test]
fn validate_create_input_rejects_empty_title() {
    let error = Handle::validate_create_input(&CreateReminderInput::new("   ")).unwrap_err();
    assert!(matches!(error, Error::InvalidReminderInput(_)));
}

#[test]
fn validate_create_input_rejects_unwritable_recurrence_fields() {
    let input = CreateReminderInput::new("Ship API").with_recurrence_rules(vec![
        RecurrenceRule::weekly(2)
            .on_days_of_week([RecurrenceDayOfWeek::every(Weekday::Monday)])
            .starting_week_on(Weekday::Sunday),
    ]);

    let error = Handle::validate_create_input(&input).unwrap_err();
    assert!(matches!(error, Error::InvalidReminderInput(_)));
}

#[test]
fn validate_reminder_query_rejects_reversed_ranges() {
    let from = DateTime::parse_from_rfc3339("2026-04-18T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let to = DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let error =
        Handle::validate_reminder_query(&ReminderFilter::completed().between(Some(from), Some(to)))
            .unwrap_err();
    assert!(matches!(error, Error::InvalidDateRange));
}

#[test]
fn parse_read_path_defaults_to_incomplete_list_items() {
    let parsed = AppleReadPath::parse("lists/test-list").unwrap();

    match parsed {
        AppleReadPath::Reminders { list_id, kind } => {
            assert_eq!(list_id.as_deref(), Some("test-list"));
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
            assert_eq!(list_id.as_deref(), Some("test-list"));
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
            assert_eq!(list_id.as_deref(), Some("test-list"));
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
fn parse_read_path_accepts_global_reminder_collections() {
    let parsed = AppleReadPath::parse("reminders/all").unwrap();

    match parsed {
        AppleReadPath::Reminders { list_id, kind } => {
            assert_eq!(list_id, None);
            assert!(matches!(kind, ReminderFilterKind::All));
        }
        AppleReadPath::Lists => panic!("expected reminders path"),
    }
}

#[test]
fn parse_read_path_rejects_unknown_paths() {
    let error = AppleReadPath::parse("lists/test-list/unknown").unwrap_err();
    assert!(matches!(error, Error::InvalidReadPath(_)));
}

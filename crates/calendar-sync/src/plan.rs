use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, FixedOffset, Utc};

use crate::types::{
    CalendarKey, ConnectionKey, IncomingCalendar, IncomingEvent, PersistedCalendar, PersistedEvent,
    SyncRange,
};

#[derive(Debug, Clone)]
pub enum CalendarOp<'a> {
    Delete {
        id: String,
    },
    Upsert {
        existing_id: Option<String>,
        incoming: &'a IncomingCalendar,
    },
}

#[derive(Debug, Clone)]
pub struct CalendarPlan<'a> {
    pub ops: Vec<CalendarOp<'a>>,
    pub enabled_calendar_ids: BTreeSet<String>,
    pub enabled_calendar_keys: BTreeMap<CalendarKey, String>,
    pub disabled_calendar_ids: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub enum EventOp<'a> {
    Delete {
        id: String,
    },
    Update {
        id: String,
        incoming: &'a IncomingEvent,
    },
    Insert {
        incoming: &'a IncomingEvent,
    },
}

#[derive(Debug, Clone)]
pub struct EventPlan<'a> {
    pub ops: Vec<EventOp<'a>>,
}

pub fn plan_calendars<'a, C: PersistedCalendar>(
    existing: &'a [C],
    incoming: &'a [IncomingCalendar],
    requested_connections: &'a BTreeSet<ConnectionKey>,
    successful_calendar_connections: &'a BTreeSet<ConnectionKey>,
) -> CalendarPlan<'a> {
    debug_assert!(incoming.iter().all(|calendar| {
        successful_calendar_connections.contains(&calendar.key.connection_key())
    }));

    let incoming_by_key: BTreeMap<_, _> =
        incoming.iter().map(|row| (row.key.clone(), row)).collect();
    let existing_by_key: BTreeMap<_, _> = existing.iter().map(|row| (row.key(), row)).collect();
    let mut ops = Vec::new();
    let mut enabled_calendar_ids = BTreeSet::new();
    let mut enabled_calendar_keys = BTreeMap::new();
    let mut disabled_calendar_ids = BTreeSet::new();

    for calendar in existing {
        let key = calendar.key();
        let connection_key = key.connection_key();
        let existing_id = calendar.id().to_string();

        let should_remove = !requested_connections.contains(&connection_key)
            || (successful_calendar_connections.contains(&connection_key)
                && !incoming_by_key.contains_key(&key));

        if should_remove {
            disabled_calendar_ids.insert(existing_id.clone());
            ops.push(CalendarOp::Delete { id: existing_id });
            continue;
        }

        if calendar.enabled() {
            enabled_calendar_ids.insert(existing_id.clone());
            enabled_calendar_keys.insert(key.clone(), existing_id.clone());
        } else {
            disabled_calendar_ids.insert(existing_id.clone());
        }

        if let Some(incoming_calendar) = incoming_by_key.get(&key) {
            ops.push(CalendarOp::Upsert {
                existing_id: Some(existing_id),
                incoming: incoming_calendar,
            });
        }
    }

    for (key, incoming_calendar) in &incoming_by_key {
        if !existing_by_key.contains_key(key) {
            ops.push(CalendarOp::Upsert {
                existing_id: None,
                incoming: incoming_calendar,
            });
        }
    }

    CalendarPlan {
        ops,
        enabled_calendar_ids,
        enabled_calendar_keys,
        disabled_calendar_ids,
    }
}

pub fn plan_events<'a, 'b, C: PersistedCalendar, E: PersistedEvent>(
    existing_calendars: &'a [C],
    existing_events: &'a [E],
    incoming: &'a [IncomingEvent],
    successful_event_connections: &'a BTreeSet<ConnectionKey>,
    calendar_plan: &'b CalendarPlan<'a>,
    range: SyncRange,
) -> EventPlan<'a> {
    debug_assert!(incoming.iter().all(|event| {
        successful_event_connections.contains(&event.calendar_key.connection_key())
    }));

    let existing_calendars_by_id: BTreeMap<_, _> = existing_calendars
        .iter()
        .map(|calendar| (calendar.id().to_string(), calendar))
        .collect();
    let mut incoming_by_identity = BTreeMap::new();
    for event in incoming {
        let identity = event_identity(&event.calendar_key, &event.tracking_id_event);
        if incoming_by_identity.insert(identity, event).is_some() {
            tracing::debug!(
                provider = ?event.calendar_key.provider,
                connection_id = %event.calendar_key.connection_id,
                tracking_id_event = %event.tracking_id_event,
                "collapsing duplicate incoming event identity"
            );
        }
    }

    let mut ops = Vec::new();
    let mut handled_identities = BTreeSet::new();

    for event in existing_events {
        if calendar_plan
            .disabled_calendar_ids
            .contains(event.calendar_id())
        {
            ops.push(EventOp::Delete {
                id: event.id().to_string(),
            });
            continue;
        }

        if !calendar_plan
            .enabled_calendar_ids
            .contains(event.calendar_id())
        {
            continue;
        }

        let Some(calendar) = existing_calendars_by_id.get(event.calendar_id()) else {
            continue;
        };
        let connection_key = calendar.key().connection_key();
        if !successful_event_connections.contains(&connection_key) {
            continue;
        }

        if !is_event_in_range(event.started_at(), event.ended_at(), range) {
            continue;
        }

        let Some(tracking_id) = event.tracking_id_event() else {
            ops.push(EventOp::Delete {
                id: event.id().to_string(),
            });
            continue;
        };
        let identity = event_identity(&calendar.key(), tracking_id);

        if let Some(incoming_event) = incoming_by_identity.get(&identity) {
            ops.push(EventOp::Update {
                id: event.id().to_string(),
                incoming: incoming_event,
            });
            handled_identities.insert(identity);
            continue;
        }

        ops.push(EventOp::Delete {
            id: event.id().to_string(),
        });
    }

    for (identity, incoming_event) in incoming_by_identity {
        if handled_identities.contains(&identity) {
            continue;
        }
        if !successful_event_connections.contains(&incoming_event.calendar_key.connection_key()) {
            continue;
        }
        if !calendar_plan
            .enabled_calendar_keys
            .contains_key(&incoming_event.calendar_key)
        {
            continue;
        }

        ops.push(EventOp::Insert {
            incoming: incoming_event,
        });
    }

    EventPlan { ops }
}

fn is_event_in_range(started_at: &str, ended_at: Option<&str>, range: SyncRange) -> bool {
    let Ok(event_start) = parse_rfc3339(started_at) else {
        return false;
    };
    let event_end = ended_at
        .and_then(|value| parse_rfc3339(value).ok())
        .unwrap_or(event_start);

    event_start <= range.to && event_end >= range.from
}

fn parse_rfc3339(value: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
    DateTime::parse_from_rfc3339(value)
        .map(|parsed: DateTime<FixedOffset>| parsed.with_timezone(&Utc))
}

fn event_identity(calendar_key: &CalendarKey, tracking_id_event: &str) -> (ConnectionKey, String) {
    (calendar_key.connection_key(), tracking_id_event.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use hypr_calendar_interface::CalendarProviderType;

    use super::*;
    use crate::types::{EventPayload, IncomingParticipant};

    #[derive(Clone)]
    struct TestCalendar {
        id: String,
        key: CalendarKey,
        enabled: bool,
    }

    impl PersistedCalendar for TestCalendar {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> CalendarKey {
            self.key.clone()
        }

        fn enabled(&self) -> bool {
            self.enabled
        }
    }

    #[derive(Clone)]
    struct TestEvent {
        id: String,
        tracking_id_event: Option<String>,
        calendar_id: String,
        started_at: String,
        ended_at: Option<String>,
    }

    impl PersistedEvent for TestEvent {
        fn id(&self) -> &str {
            &self.id
        }

        fn tracking_id_event(&self) -> Option<&str> {
            self.tracking_id_event.as_deref()
        }

        fn calendar_id(&self) -> &str {
            &self.calendar_id
        }

        fn started_at(&self) -> &str {
            &self.started_at
        }

        fn ended_at(&self) -> Option<&str> {
            self.ended_at.as_deref()
        }
    }

    #[test]
    fn overlapping_event_ids_update_within_their_connection() {
        let calendars = vec![
            test_calendar("cal-john", "conn-john", "primary", true),
            test_calendar("cal-gmail", "conn-gmail", "primary", true),
        ];
        let incoming_calendars = vec![
            test_incoming_calendar("conn-john", "primary"),
            test_incoming_calendar("conn-gmail", "primary"),
        ];
        let requested_connections = BTreeSet::from([
            ConnectionKey::new(CalendarProviderType::Google, "conn-john"),
            ConnectionKey::new(CalendarProviderType::Google, "conn-gmail"),
        ]);
        let calendar_plan = plan_calendars(
            &calendars,
            &incoming_calendars,
            &requested_connections,
            &requested_connections,
        );
        let events = vec![
            test_event("event-john", Some("evt-1"), "cal-john"),
            test_event("event-gmail", Some("evt-1"), "cal-gmail"),
        ];
        let incoming = vec![
            test_incoming_event("conn-john", "primary", "evt-1"),
            test_incoming_event("conn-gmail", "primary", "evt-1"),
        ];

        let plan = plan_events(
            &calendars,
            &events,
            &incoming,
            &requested_connections,
            &calendar_plan,
            test_range(),
        );

        assert_eq!(plan.ops.len(), 2);
        match &plan.ops[0] {
            EventOp::Update { id, incoming } => {
                assert_eq!(id, "event-john");
                assert_eq!(incoming.calendar_key.connection_id, "conn-john");
            }
            other => panic!("expected update for john, got {other:?}"),
        }
        match &plan.ops[1] {
            EventOp::Update { id, incoming } => {
                assert_eq!(id, "event-gmail");
                assert_eq!(incoming.calendar_key.connection_id, "conn-gmail");
            }
            other => panic!("expected update for gmail, got {other:?}"),
        }
    }

    #[test]
    fn missing_event_deletes_only_for_successful_connection() {
        let calendars = vec![
            test_calendar("cal-john", "conn-john", "primary", true),
            test_calendar("cal-gmail", "conn-gmail", "primary", true),
        ];
        let incoming_calendars = vec![
            test_incoming_calendar("conn-john", "primary"),
            test_incoming_calendar("conn-gmail", "primary"),
        ];
        let requested_connections = BTreeSet::from([
            ConnectionKey::new(CalendarProviderType::Google, "conn-john"),
            ConnectionKey::new(CalendarProviderType::Google, "conn-gmail"),
        ]);
        let successful_event_connections = BTreeSet::from([ConnectionKey::new(
            CalendarProviderType::Google,
            "conn-john",
        )]);
        let calendar_plan = plan_calendars(
            &calendars,
            &incoming_calendars,
            &requested_connections,
            &requested_connections,
        );
        let events = vec![
            test_event("event-john", Some("evt-1"), "cal-john"),
            test_event("event-gmail", Some("evt-1"), "cal-gmail"),
        ];

        let plan = plan_events(
            &calendars,
            &events,
            &[],
            &successful_event_connections,
            &calendar_plan,
            test_range(),
        );

        assert_eq!(plan.ops.len(), 1);
        match &plan.ops[0] {
            EventOp::Delete { id } => assert_eq!(id, "event-john"),
            other => panic!("expected delete, got {other:?}"),
        }
    }

    #[test]
    fn inserts_only_for_matching_enabled_connection() {
        let calendars = vec![
            test_calendar("cal-john", "conn-john", "primary", true),
            test_calendar("cal-gmail", "conn-gmail", "primary", false),
        ];
        let incoming_calendars = vec![
            test_incoming_calendar("conn-john", "primary"),
            test_incoming_calendar("conn-gmail", "primary"),
        ];
        let requested_connections = BTreeSet::from([
            ConnectionKey::new(CalendarProviderType::Google, "conn-john"),
            ConnectionKey::new(CalendarProviderType::Google, "conn-gmail"),
        ]);
        let calendar_plan = plan_calendars(
            &calendars,
            &incoming_calendars,
            &requested_connections,
            &requested_connections,
        );
        let incoming = vec![
            test_incoming_event("conn-john", "primary", "evt-1"),
            test_incoming_event("conn-gmail", "primary", "evt-2"),
        ];
        let events = Vec::<TestEvent>::new();

        let plan = plan_events(
            &calendars,
            &events,
            &incoming,
            &requested_connections,
            &calendar_plan,
            test_range(),
        );

        assert_eq!(plan.ops.len(), 1);
        match &plan.ops[0] {
            EventOp::Insert { incoming } => {
                assert_eq!(incoming.calendar_key.connection_id, "conn-john");
                assert_eq!(incoming.tracking_id_event, "evt-1");
            }
            other => panic!("expected insert, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_incoming_identity_inserts_once() {
        let calendars = vec![test_calendar("cal-john", "conn-john", "primary", true)];
        let incoming_calendars = vec![test_incoming_calendar("conn-john", "primary")];
        let requested_connections = BTreeSet::from([ConnectionKey::new(
            CalendarProviderType::Google,
            "conn-john",
        )]);
        let calendar_plan = plan_calendars(
            &calendars,
            &incoming_calendars,
            &requested_connections,
            &requested_connections,
        );
        let incoming = vec![
            test_incoming_event("conn-john", "primary", "evt-1"),
            IncomingEvent {
                payload: EventPayload {
                    title: Some("latest title wins".to_string()),
                    ..EventPayload::default()
                },
                ..test_incoming_event("conn-john", "primary", "evt-1")
            },
        ];
        let events = Vec::<TestEvent>::new();

        let plan = plan_events(
            &calendars,
            &events,
            &incoming,
            &requested_connections,
            &calendar_plan,
            test_range(),
        );

        assert_eq!(plan.ops.len(), 1);
        match &plan.ops[0] {
            EventOp::Insert { incoming } => {
                assert_eq!(incoming.calendar_key.connection_id, "conn-john");
                assert_eq!(incoming.tracking_id_event, "evt-1");
                assert_eq!(incoming.payload.title.as_deref(), Some("latest title wins"));
            }
            other => panic!("expected insert, got {other:?}"),
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn incoming_calendars_must_belong_to_successful_connections() {
        let requested_connections = BTreeSet::from([ConnectionKey::new(
            CalendarProviderType::Google,
            "conn-john",
        )]);

        let existing = Vec::<TestCalendar>::new();

        let _ = plan_calendars(
            &existing,
            &[test_incoming_calendar("conn-john", "primary")],
            &requested_connections,
            &BTreeSet::new(),
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn incoming_events_must_belong_to_successful_connections() {
        let calendars = vec![test_calendar("cal-john", "conn-john", "primary", true)];
        let requested_connections = BTreeSet::from([ConnectionKey::new(
            CalendarProviderType::Google,
            "conn-john",
        )]);
        let incoming_calendars = vec![test_incoming_calendar("conn-john", "primary")];
        let calendar_plan = plan_calendars(
            &calendars,
            &incoming_calendars,
            &requested_connections,
            &requested_connections,
        );

        let events = Vec::<TestEvent>::new();

        let _ = plan_events(
            &calendars,
            &events,
            &[test_incoming_event("conn-john", "primary", "evt-1")],
            &BTreeSet::new(),
            &calendar_plan,
            test_range(),
        );
    }

    fn test_calendar(
        id: &str,
        connection_id: &str,
        tracking_id: &str,
        enabled: bool,
    ) -> TestCalendar {
        TestCalendar {
            id: id.to_string(),
            key: CalendarKey::new(CalendarProviderType::Google, connection_id, tracking_id),
            enabled,
        }
    }

    fn test_event(id: &str, tracking_id_event: Option<&str>, calendar_id: &str) -> TestEvent {
        TestEvent {
            id: id.to_string(),
            tracking_id_event: tracking_id_event.map(ToString::to_string),
            calendar_id: calendar_id.to_string(),
            started_at: "2026-04-10T10:00:00Z".to_string(),
            ended_at: Some("2026-04-10T11:00:00Z".to_string()),
        }
    }

    fn test_incoming_calendar(connection_id: &str, tracking_id: &str) -> IncomingCalendar {
        IncomingCalendar {
            key: CalendarKey::new(CalendarProviderType::Google, connection_id, tracking_id),
            payload: crate::types::CalendarPayload {
                name: tracking_id.to_string(),
                source: connection_id.to_string(),
                color: "#888".to_string(),
            },
        }
    }

    fn test_incoming_event(
        connection_id: &str,
        calendar_tracking_id: &str,
        tracking_id_event: &str,
    ) -> IncomingEvent {
        IncomingEvent {
            calendar_key: CalendarKey::new(
                CalendarProviderType::Google,
                connection_id,
                calendar_tracking_id,
            ),
            tracking_id_event: tracking_id_event.to_string(),
            started_at: "2026-04-10T10:00:00Z".to_string(),
            ended_at: Some("2026-04-10T11:00:00Z".to_string()),
            recurrence_series_id: None,
            has_recurrence_rules: false,
            is_all_day: false,
            participants: Vec::<IncomingParticipant>::new(),
            payload: EventPayload::default(),
        }
    }

    fn test_range() -> SyncRange {
        SyncRange {
            from: Utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0).unwrap(),
            to: Utc.with_ymd_and_hms(2026, 4, 30, 23, 59, 59).unwrap(),
        }
    }
}

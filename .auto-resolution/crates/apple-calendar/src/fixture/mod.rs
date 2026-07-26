use std::sync::RwLock;

use json_patch::{Patch, patch};
use strum::{AsRefStr, EnumString, VariantNames};

use crate::types::{AppleCalendar, AppleEvent, EventFilter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, EnumString, AsRefStr, VariantNames)]
#[strum(serialize_all = "snake_case")]
pub enum FixtureBase {
    #[default]
    Default,
}

pub const STEP_NAMES: &[&str] = &["Base", "Event Added", "Event Removed", "Event Rescheduled"];

static CURRENT_BASE: RwLock<FixtureBase> = RwLock::new(FixtureBase::Default);
static CURRENT_STEP: RwLock<usize> = RwLock::new(0);

pub fn set_base(base: FixtureBase) {
    if let Ok(mut current) = CURRENT_BASE.write() {
        *current = base;
    }
}

pub fn get_base() -> FixtureBase {
    CURRENT_BASE.read().map(|b| *b).unwrap_or_default()
}

pub fn list_bases() -> &'static [&'static str] {
    FixtureBase::VARIANTS
}

pub fn get_step() -> usize {
    CURRENT_STEP.read().map(|s| *s).unwrap_or(0)
}

pub fn get_max_steps() -> usize {
    STEP_NAMES.len()
}

pub fn advance_step() -> usize {
    if let Ok(mut step) = CURRENT_STEP.write() {
        if *step < STEP_NAMES.len() - 1 {
            *step += 1;
        }
        *step
    } else {
        0
    }
}

pub fn reset_step() {
    if let Ok(mut step) = CURRENT_STEP.write() {
        *step = 0;
    }
}

pub fn get_step_name(step: usize) -> &'static str {
    STEP_NAMES.get(step).unwrap_or(&"Unknown")
}

macro_rules! include_base_calendars {
    (Default) => {
        include_str!("data/default/base/calendars.json")
    };
}

macro_rules! include_base_events {
    (Default) => {
        include_str!("data/default/base/events.json")
    };
}

fn load_calendars(base: FixtureBase) -> Vec<AppleCalendar> {
    let data = match base {
        FixtureBase::Default => include_base_calendars!(Default),
    };
    serde_json::from_str(data).expect("Failed to parse fixture calendars.json")
}

fn load_base_events(base: FixtureBase) -> serde_json::Value {
    let data = match base {
        FixtureBase::Default => include_base_events!(Default),
    };
    serde_json::from_str(data).expect("Failed to parse base events.json")
}

fn ordered_patches(base: FixtureBase) -> Vec<Patch> {
    match base {
        FixtureBase::Default => vec![
            serde_json::from_str(include_str!("data/default/patch/event_added.json"))
                .expect("Failed to parse event_added.json"),
            serde_json::from_str(include_str!("data/default/patch/event_removed.json"))
                .expect("Failed to parse event_removed.json"),
            serde_json::from_str(include_str!("data/default/patch/event_rescheduled.json"))
                .expect("Failed to parse event_rescheduled.json"),
        ],
    }
}

fn load_events(base: FixtureBase, step: usize) -> Vec<AppleEvent> {
    let mut events = load_base_events(base);
    let patches = ordered_patches(base);

    for p in patches.iter().take(step) {
        patch(&mut events, p).expect("Failed to apply patch");
    }

    serde_json::from_value(events).expect("Failed to deserialize patched events")
}

pub fn list_calendars() -> Result<Vec<AppleCalendar>, String> {
    let base = get_base();
    Ok(load_calendars(base))
}

pub fn list_events(filter: EventFilter) -> Result<Vec<AppleEvent>, String> {
    let base = get_base();
    let step = get_step();
    let all_events = load_events(base, step);

    let filtered_events: Vec<AppleEvent> = all_events
        .into_iter()
        .filter(|event| {
            event.calendar.id == filter.calendar_tracking_id
                && event.start_date >= filter.from
                && event.start_date <= filter.to
        })
        .collect();

    Ok(filtered_events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_fixture_calendars() {
        let calendars = list_calendars().unwrap();
        assert!(!calendars.is_empty());
        assert_eq!(calendars[0].id, "fixture-calendar-1");
    }

    #[test]
    fn test_step_0_base_has_two_events() {
        let events = load_events(FixtureBase::Default, 0);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_identifier, "fixture-event-1");
        assert_eq!(events[1].event_identifier, "fixture-event-2");
    }

    #[test]
    fn test_step_1_event_added_has_three_events() {
        let events = load_events(FixtureBase::Default, 1);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_identifier, "fixture-event-1");
        assert_eq!(events[1].event_identifier, "fixture-event-2");
        assert_eq!(events[2].event_identifier, "fixture-event-3");
        assert_eq!(events[2].title, "New Client Call");
    }

    #[test]
    fn test_step_2_event_removed_has_two_events() {
        let events = load_events(FixtureBase::Default, 2);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_identifier, "fixture-event-1");
        assert_eq!(events[1].event_identifier, "fixture-event-3");
    }

    #[test]
    fn test_step_3_event_rescheduled() {
        let events = load_events(FixtureBase::Default, 3);
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0].start_date.to_rfc3339(),
            "2025-01-02T10:00:00+00:00"
        );
        assert_eq!(events[0].notes.as_deref(), Some("Rescheduled standup"));
        assert_eq!(
            events[1].start_date.to_rfc3339(),
            "2025-01-03T16:00:00+00:00"
        );
    }

    #[test]
    fn test_advance_step() {
        reset_step();
        assert_eq!(get_step(), 0);

        assert_eq!(advance_step(), 1);
        assert_eq!(get_step(), 1);

        assert_eq!(advance_step(), 2);
        assert_eq!(advance_step(), 3);
        assert_eq!(advance_step(), 3);

        reset_step();
        assert_eq!(get_step(), 0);
    }

    #[test]
    fn test_get_max_steps() {
        assert_eq!(get_max_steps(), 4);
    }

    #[test]
    fn test_get_step_name() {
        assert_eq!(get_step_name(0), "Base");
        assert_eq!(get_step_name(1), "Event Added");
        assert_eq!(get_step_name(2), "Event Removed");
        assert_eq!(get_step_name(3), "Event Rescheduled");
    }

    #[test]
    fn test_switch_base() {
        set_base(FixtureBase::Default);
        assert_eq!(get_base(), FixtureBase::Default);
    }

    #[test]
    fn test_list_bases() {
        let bases = list_bases();
        assert!(bases.contains(&"default"));
    }

    mod schema_validation {
        use jsonschema::Validator;
        use schemars::schema_for;

        use super::*;
        use crate::types::{AppleCalendar, AppleEvent};

        fn calendars_schema() -> serde_json::Value {
            let schema = schema_for!(Vec<AppleCalendar>);
            serde_json::to_value(schema).expect("Failed to serialize calendars schema")
        }

        fn events_schema() -> serde_json::Value {
            let schema = schema_for!(Vec<AppleEvent>);
            serde_json::to_value(schema).expect("Failed to serialize events schema")
        }

        fn assert_valid(validator: &Validator, data: &serde_json::Value, context: &str) {
            let errors: Vec<String> = validator.iter_errors(data).map(|e| e.to_string()).collect();
            assert!(
                errors.is_empty(),
                "{} failed schema validation:\n{}",
                context,
                errors.join("\n")
            );
        }

        macro_rules! schema_file_test {
            ($name:ident, $schema:expr, $json_path:literal, $label:literal) => {
                #[test]
                fn $name() {
                    let validator = Validator::new(&$schema).expect("Failed to compile schema");
                    let data: serde_json::Value = serde_json::from_str(include_str!($json_path))
                        .expect(concat!("Failed to parse ", $label));
                    assert_valid(&validator, &data, $label);
                }
            };
        }

        schema_file_test!(
            test_base_calendars,
            calendars_schema(),
            "data/default/base/calendars.json",
            "base calendars"
        );

        schema_file_test!(
            test_base_events,
            events_schema(),
            "data/default/base/events.json",
            "base events"
        );

        #[test]
        fn test_all_cumulative_steps_valid() {
            let validator =
                Validator::new(&events_schema()).expect("Failed to compile events schema");

            for step in 0..=3 {
                let events = load_events(FixtureBase::Default, step);
                let data = serde_json::to_value(&events).expect("Failed to serialize events");
                assert_valid(&validator, &data, &format!("step {}", step));
            }
        }
    }
}

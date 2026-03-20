use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct EventFilter {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub calendar_tracking_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreateEventInput {
    pub title: String,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub calendar_id: String,
    pub is_all_day: Option<bool>,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub url: Option<String>,
}

macro_rules! common_derives {
    ($item:item) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
        #[cfg_attr(feature = "specta", derive(specta::Type))]
        $item
    };
}

common_derives! {
    pub struct CalendarColor {
        pub red: f32,
        pub green: f32,
        pub blue: f32,
        pub alpha: f32,
    }
}

common_derives! {
    pub enum Weekday {
        Sunday,
        Monday,
        Tuesday,
        Wednesday,
        Thursday,
        Friday,
        Saturday,
    }
}

common_derives! {
    pub enum CalendarSourceType {
        Local,
        Exchange,
        CalDav,
        MobileMe,
        Subscribed,
        Birthdays,
    }
}

common_derives! {
    pub enum CalendarType {
        Local,
        CalDav,
        Exchange,
        Subscription,
        Birthday,
    }
}

common_derives! {
    pub enum CalendarEntityType {
        Event,
        Reminder,
    }
}

common_derives! {
    pub struct CalendarSource {
        pub identifier: String,
        pub title: String,
        pub source_type: CalendarSourceType,
    }
}

impl Default for CalendarSource {
    fn default() -> Self {
        Self {
            identifier: String::new(),
            title: String::new(),
            source_type: CalendarSourceType::Local,
        }
    }
}

common_derives! {
    pub struct AppleCalendar {
        pub id: String,
        pub title: String,
        pub calendar_type: CalendarType,
        pub color: Option<CalendarColor>,
        pub allows_content_modifications: bool,
        pub is_immutable: bool,
        pub is_subscribed: bool,
        pub supported_event_availabilities: Vec<EventAvailability>,
        pub allowed_entity_types: Vec<CalendarEntityType>,
        pub source: CalendarSource,
    }
}

common_derives! {
    pub enum EventAvailability {
        NotSupported,
        Busy,
        Free,
        Tentative,
        Unavailable,
    }
}

common_derives! {
    pub enum EventStatus {
        None,
        Confirmed,
        Tentative,
        Canceled,
    }
}

common_derives! {
    pub enum AlarmProximity {
        None,
        Enter,
        Leave,
    }
}

common_derives! {
    pub enum AlarmType {
        Display,
        Audio,
        Procedure,
        Email,
    }
}

common_derives! {
    pub struct GeoLocation {
        pub latitude: f64,
        pub longitude: f64,
    }
}

common_derives! {
    pub struct StructuredLocation {
        pub title: String,
        pub geo: Option<GeoLocation>,
        pub radius: Option<f64>,
    }
}

common_derives! {
    pub struct Alarm {
        pub absolute_date: Option<DateTime<Utc>>,
        pub relative_offset: Option<f64>,
        pub proximity: Option<AlarmProximity>,
        pub alarm_type: Option<AlarmType>,
        pub email_address: Option<String>,
        pub sound_name: Option<String>,
        pub url: Option<String>,
        pub structured_location: Option<StructuredLocation>,
    }
}

common_derives! {
    pub enum ParticipantRole {
        Unknown,
        Required,
        Optional,
        Chair,
        NonParticipant,
    }
}

common_derives! {
    pub enum ParticipantStatus {
        Unknown,
        Pending,
        Accepted,
        Declined,
        Tentative,
        Delegated,
        Completed,
        InProgress,
    }
}

common_derives! {
    pub enum ParticipantType {
        Unknown,
        Person,
        Room,
        Resource,
        Group,
    }
}

common_derives! {
    pub struct ParticipantContact {
        pub identifier: String,
        pub given_name: Option<String>,
        pub family_name: Option<String>,
        pub middle_name: Option<String>,
        pub organization_name: Option<String>,
        pub job_title: Option<String>,
        pub email_addresses: Vec<String>,
        pub phone_numbers: Vec<String>,
        pub url_addresses: Vec<String>,
        pub image_available: bool,
    }
}

common_derives! {
    pub struct Participant {
        pub name: Option<String>,
        pub email: Option<String>,
        pub is_current_user: bool,
        pub role: ParticipantRole,
        pub status: ParticipantStatus,
        pub participant_type: ParticipantType,

        pub url: Option<String>,
        pub contact: Option<ParticipantContact>,
    }
}

common_derives! {
    pub enum RecurrenceFrequency {
        Daily,
        Weekly,
        Monthly,
        Yearly,
    }
}

common_derives! {
    pub enum RecurrenceEnd {
        Count(u32),
        Until(DateTime<Utc>),
    }
}

common_derives! {
    pub struct RecurrenceDayOfWeek {
        pub weekday: Weekday,
        pub week_number: Option<i8>,
    }
}

common_derives! {
    pub struct RecurrenceRule {
        pub frequency: RecurrenceFrequency,
        pub interval: u32,
        pub days_of_week: Vec<RecurrenceDayOfWeek>,
        pub days_of_month: Vec<i8>,
        pub months_of_year: Vec<u8>,
        pub weeks_of_year: Vec<i8>,
        pub days_of_year: Vec<i16>,
        pub set_positions: Vec<i16>,
        pub first_day_of_week: Option<Weekday>,
        pub end: Option<RecurrenceEnd>,
    }
}

common_derives! {
    pub struct RecurrenceOccurrence {
        pub original_start: DateTime<Utc>,
        pub is_detached: bool,
    }
}

common_derives! {
    pub struct RecurrenceInfo {
        pub series_identifier: String,
        pub has_recurrence_rules: bool,
        pub occurrence: Option<RecurrenceOccurrence>,
        pub rules: Vec<RecurrenceRule>,
    }
}

common_derives! {
    pub struct CalendarRef {
        pub id: String,
        pub title: String,
    }
}

common_derives! {
    pub struct AppleEvent {
        pub event_identifier: String,
        pub calendar_item_identifier: String,
        pub external_identifier: String,
        pub calendar: CalendarRef,
        pub title: String,
        pub location: Option<String>,
        pub url: Option<String>,
        pub notes: Option<String>,
        pub creation_date: Option<DateTime<Utc>>,
        pub last_modified_date: Option<DateTime<Utc>>,
        pub time_zone: Option<String>,
        pub start_date: DateTime<Utc>,
        pub end_date: DateTime<Utc>,
        pub is_all_day: bool,
        pub availability: EventAvailability,
        pub status: EventStatus,
        pub has_alarms: bool,
        pub has_attendees: bool,
        pub has_notes: bool,
        pub has_recurrence_rules: bool,
        pub organizer: Option<Participant>,
        pub attendees: Vec<Participant>,
        pub structured_location: Option<StructuredLocation>,
        pub recurrence: Option<RecurrenceInfo>,
        pub occurrence_date: Option<DateTime<Utc>>,
        pub is_detached: bool,
        pub alarms: Vec<Alarm>,
        pub birthday_contact_identifier: Option<String>,
        pub is_birthday: bool,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::schema_for;
    use std::fs;
    use std::path::PathBuf;

    fn get_schemas_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/fixture/schema")
    }

    #[test]
    #[ignore]
    fn generate_schemas() {
        let schemas_dir = get_schemas_dir();
        fs::create_dir_all(&schemas_dir).expect("Failed to create schemas directory");

        let calendars_schema = schema_for!(Vec<AppleCalendar>);
        let calendars_json =
            serde_json::to_string_pretty(&calendars_schema).expect("Failed to serialize schema");
        fs::write(schemas_dir.join("calendars.schema.json"), calendars_json)
            .expect("Failed to write calendars schema");

        let events_schema = schema_for!(Vec<AppleEvent>);
        let events_json =
            serde_json::to_string_pretty(&events_schema).expect("Failed to serialize schema");
        fs::write(schemas_dir.join("events.schema.json"), events_json)
            .expect("Failed to write events schema");
    }
}

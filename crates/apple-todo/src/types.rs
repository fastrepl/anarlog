use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};

macro_rules! common_derives {
    ($item:item) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
        #[cfg_attr(feature = "specta", derive(specta::Type))]
        $item
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ReminderFilter {
    pub kind: ReminderFilterKind,
    pub list_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ReminderFilterKind {
    All,
    Incomplete {
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    },
    Completed {
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreateReminderInput {
    pub title: String,
    pub list_id: Option<String>,
    pub notes: Option<String>,
    pub url: Option<String>,
    pub priority: Option<ReminderPriority>,
    pub due_date: Option<DateTime<Utc>>,
    pub start_date: Option<DateTime<Utc>>,
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
    pub struct ReminderListRef {
        pub id: String,
        pub title: String,
    }
}

common_derives! {
    pub struct ReminderList {
        pub id: String,
        pub title: String,
        pub calendar_type: CalendarType,
        pub color: Option<CalendarColor>,
        pub allows_content_modifications: bool,
        pub is_default: bool,
        pub source: CalendarSource,
    }
}

common_derives! {
    pub enum ReminderPriority {
        None,
        High,
        Medium,
        Low,
    }
}

impl ReminderPriority {
    pub fn from_native(value: i64) -> Self {
        match value {
            1..=4 => ReminderPriority::High,
            5 => ReminderPriority::Medium,
            6..=9 => ReminderPriority::Low,
            _ => ReminderPriority::None,
        }
    }

    pub fn to_native(&self) -> i64 {
        match self {
            ReminderPriority::None => 0,
            ReminderPriority::High => 1,
            ReminderPriority::Medium => 5,
            ReminderPriority::Low => 9,
        }
    }
}

common_derives! {
    pub struct DateComponents {
        pub date: Option<NaiveDate>,
        pub time: Option<NaiveTime>,
        pub time_zone: Option<String>,
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
    pub struct Reminder {
        pub calendar_item_identifier: String,
        pub external_identifier: String,
        pub list: ReminderListRef,
        pub title: String,
        pub notes: Option<String>,
        pub url: Option<String>,
        pub priority: ReminderPriority,
        pub is_completed: bool,
        pub completion_date: Option<DateTime<Utc>>,
        pub start_date_components: Option<DateComponents>,
        pub due_date_components: Option<DateComponents>,
        pub creation_date: Option<DateTime<Utc>>,
        pub last_modified_date: Option<DateTime<Utc>>,
        pub has_alarms: bool,
        pub has_recurrence_rules: bool,
        pub alarms: Vec<Alarm>,
        pub recurrence_rules: Vec<RecurrenceRule>,
    }
}

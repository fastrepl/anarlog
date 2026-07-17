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

impl ReminderFilter {
    pub fn all() -> Self {
        Self {
            kind: ReminderFilterKind::All,
            list_ids: None,
        }
    }

    pub fn incomplete() -> Self {
        Self {
            kind: ReminderFilterKind::Incomplete {
                from: None,
                to: None,
            },
            list_ids: None,
        }
    }

    pub fn completed() -> Self {
        Self {
            kind: ReminderFilterKind::Completed {
                from: None,
                to: None,
            },
            list_ids: None,
        }
    }

    pub fn in_list(mut self, list_id: impl Into<String>) -> Self {
        self.list_ids = Some(vec![list_id.into()]);
        self
    }

    pub fn in_lists<I, S>(mut self, list_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.list_ids = Some(list_ids.into_iter().map(Into::into).collect());
        self
    }

    pub fn between(mut self, from: Option<DateTime<Utc>>, to: Option<DateTime<Utc>>) -> Self {
        match &mut self.kind {
            ReminderFilterKind::All => {}
            ReminderFilterKind::Incomplete {
                from: filter_from,
                to: filter_to,
            }
            | ReminderFilterKind::Completed {
                from: filter_from,
                to: filter_to,
            } => {
                *filter_from = from;
                *filter_to = to;
            }
        }
        self
    }
}

pub type ReminderQuery = ReminderFilter;

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

pub type ReminderQueryKind = ReminderFilterKind;

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

impl DateComponents {
    pub fn all_day(date: NaiveDate) -> Self {
        Self {
            date: Some(date),
            time: None,
            time_zone: None,
        }
    }

    pub fn floating(date: NaiveDate, time: NaiveTime) -> Self {
        Self {
            date: Some(date),
            time: Some(time),
            time_zone: None,
        }
    }

    pub fn zoned(date: NaiveDate, time: NaiveTime, time_zone: impl Into<String>) -> Self {
        Self {
            date: Some(date),
            time: Some(time),
            time_zone: Some(time_zone.into()),
        }
    }

    pub fn with_time(mut self, time: NaiveTime) -> Self {
        self.time = Some(time);
        self
    }

    pub fn with_time_zone(mut self, time_zone: impl Into<String>) -> Self {
        self.time_zone = Some(time_zone.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreateReminderInput {
    pub title: String,
    pub list_id: Option<String>,
    pub notes: Option<String>,
    pub url: Option<String>,
    pub priority: Option<ReminderPriority>,
    pub due_date: Option<DateComponents>,
    pub start_date: Option<DateComponents>,
    pub alarms: Option<Vec<Alarm>>,
    pub recurrence_rules: Option<Vec<RecurrenceRule>>,
    pub is_completed: Option<bool>,
    pub completion_date: Option<DateTime<Utc>>,
}

impl CreateReminderInput {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            list_id: None,
            notes: None,
            url: None,
            priority: None,
            due_date: None,
            start_date: None,
            alarms: None,
            recurrence_rules: None,
            is_completed: None,
            completion_date: None,
        }
    }

    pub fn in_list(mut self, list_id: impl Into<String>) -> Self {
        self.list_id = Some(list_id.into());
        self
    }

    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn with_priority(mut self, priority: ReminderPriority) -> Self {
        self.priority = Some(priority);
        self
    }

    pub fn with_due_date(mut self, due_date: DateComponents) -> Self {
        self.due_date = Some(due_date);
        self
    }

    pub fn with_due_date_on(mut self, due_date: NaiveDate) -> Self {
        self.due_date = Some(DateComponents::all_day(due_date));
        self
    }

    pub fn with_due_date_at(
        mut self,
        due_date: NaiveDate,
        due_time: NaiveTime,
        time_zone: impl Into<String>,
    ) -> Self {
        self.due_date = Some(DateComponents::zoned(due_date, due_time, time_zone));
        self
    }

    pub fn with_start_date(mut self, start_date: DateComponents) -> Self {
        self.start_date = Some(start_date);
        self
    }

    pub fn with_start_date_on(mut self, start_date: NaiveDate) -> Self {
        self.start_date = Some(DateComponents::all_day(start_date));
        self
    }

    pub fn with_start_date_at(
        mut self,
        start_date: NaiveDate,
        start_time: NaiveTime,
        time_zone: impl Into<String>,
    ) -> Self {
        self.start_date = Some(DateComponents::zoned(start_date, start_time, time_zone));
        self
    }

    pub fn with_alarms(mut self, alarms: Vec<Alarm>) -> Self {
        self.alarms = Some(alarms);
        self
    }

    pub fn with_recurrence_rules(mut self, recurrence_rules: Vec<RecurrenceRule>) -> Self {
        self.recurrence_rules = Some(recurrence_rules);
        self
    }

    pub fn mark_completed(mut self) -> Self {
        self.is_completed = Some(true);
        self
    }

    pub fn with_completion_date(mut self, completion_date: DateTime<Utc>) -> Self {
        self.is_completed = Some(true);
        self.completion_date = Some(completion_date);
        self
    }
}

pub type CreateReminder = CreateReminderInput;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ReminderIdentifierInput {
    pub calendar_item_identifier: Option<String>,
    pub external_identifier: Option<String>,
    pub list_id: Option<String>,
}

impl ReminderIdentifierInput {
    pub fn by_calendar_item_identifier(calendar_item_identifier: impl Into<String>) -> Self {
        Self {
            calendar_item_identifier: Some(calendar_item_identifier.into()),
            external_identifier: None,
            list_id: None,
        }
    }

    pub fn by_external_identifier(external_identifier: impl Into<String>) -> Self {
        Self {
            calendar_item_identifier: None,
            external_identifier: Some(external_identifier.into()),
            list_id: None,
        }
    }

    pub fn in_list(mut self, list_id: impl Into<String>) -> Self {
        self.list_id = Some(list_id.into());
        self
    }

    pub fn from_reminder(reminder: &Reminder) -> Self {
        Self::by_calendar_item_identifier(&reminder.calendar_item_identifier)
    }
}

pub type ReminderLookup = ReminderIdentifierInput;

impl From<&Reminder> for ReminderIdentifierInput {
    fn from(reminder: &Reminder) -> Self {
        Self::from_reminder(reminder)
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

impl GeoLocation {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
        }
    }
}

common_derives! {
    pub struct StructuredLocation {
        pub title: String,
        pub geo: Option<GeoLocation>,
        pub radius: Option<f64>,
    }
}

impl StructuredLocation {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            geo: None,
            radius: None,
        }
    }

    pub fn with_geo(mut self, geo: GeoLocation) -> Self {
        self.geo = Some(geo);
        self
    }

    pub fn with_radius(mut self, radius: f64) -> Self {
        self.radius = Some(radius);
        self
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

impl Alarm {
    pub fn at(absolute_date: DateTime<Utc>) -> Self {
        Self {
            absolute_date: Some(absolute_date),
            relative_offset: None,
            proximity: None,
            alarm_type: None,
            email_address: None,
            sound_name: None,
            url: None,
            structured_location: None,
        }
    }

    pub fn relative(relative_offset: f64) -> Self {
        Self {
            absolute_date: None,
            relative_offset: Some(relative_offset),
            proximity: None,
            alarm_type: None,
            email_address: None,
            sound_name: None,
            url: None,
            structured_location: None,
        }
    }

    pub fn with_proximity(mut self, proximity: AlarmProximity) -> Self {
        self.proximity = Some(proximity);
        self
    }

    pub fn with_email_address(mut self, email_address: impl Into<String>) -> Self {
        self.email_address = Some(email_address.into());
        self
    }

    pub fn with_sound_name(mut self, sound_name: impl Into<String>) -> Self {
        self.sound_name = Some(sound_name.into());
        self
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn with_structured_location(mut self, structured_location: StructuredLocation) -> Self {
        self.structured_location = Some(structured_location);
        self
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

impl RecurrenceDayOfWeek {
    pub fn every(weekday: Weekday) -> Self {
        Self {
            weekday,
            week_number: None,
        }
    }

    pub fn nth(weekday: Weekday, week_number: i8) -> Self {
        Self {
            weekday,
            week_number: Some(week_number),
        }
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

impl RecurrenceRule {
    pub fn daily(interval: u32) -> Self {
        Self::new(RecurrenceFrequency::Daily, interval)
    }

    pub fn weekly(interval: u32) -> Self {
        Self::new(RecurrenceFrequency::Weekly, interval)
    }

    pub fn monthly(interval: u32) -> Self {
        Self::new(RecurrenceFrequency::Monthly, interval)
    }

    pub fn yearly(interval: u32) -> Self {
        Self::new(RecurrenceFrequency::Yearly, interval)
    }

    fn new(frequency: RecurrenceFrequency, interval: u32) -> Self {
        Self {
            frequency,
            interval: interval.max(1),
            days_of_week: Vec::new(),
            days_of_month: Vec::new(),
            months_of_year: Vec::new(),
            weeks_of_year: Vec::new(),
            days_of_year: Vec::new(),
            set_positions: Vec::new(),
            first_day_of_week: None,
            end: None,
        }
    }

    pub fn on_days_of_week<I>(mut self, days_of_week: I) -> Self
    where
        I: IntoIterator<Item = RecurrenceDayOfWeek>,
    {
        self.days_of_week = days_of_week.into_iter().collect();
        self
    }

    pub fn on_days_of_month<I>(mut self, days_of_month: I) -> Self
    where
        I: IntoIterator<Item = i8>,
    {
        self.days_of_month = days_of_month.into_iter().collect();
        self
    }

    pub fn in_months_of_year<I>(mut self, months_of_year: I) -> Self
    where
        I: IntoIterator<Item = u8>,
    {
        self.months_of_year = months_of_year.into_iter().collect();
        self
    }

    pub fn in_weeks_of_year<I>(mut self, weeks_of_year: I) -> Self
    where
        I: IntoIterator<Item = i8>,
    {
        self.weeks_of_year = weeks_of_year.into_iter().collect();
        self
    }

    pub fn on_days_of_year<I>(mut self, days_of_year: I) -> Self
    where
        I: IntoIterator<Item = i16>,
    {
        self.days_of_year = days_of_year.into_iter().collect();
        self
    }

    pub fn with_set_positions<I>(mut self, set_positions: I) -> Self
    where
        I: IntoIterator<Item = i16>,
    {
        self.set_positions = set_positions.into_iter().collect();
        self
    }

    pub fn starting_week_on(mut self, first_day_of_week: Weekday) -> Self {
        self.first_day_of_week = Some(first_day_of_week);
        self
    }

    pub fn ending_after(mut self, count: u32) -> Self {
        self.end = Some(RecurrenceEnd::Count(count));
        self
    }

    pub fn until(mut self, until: DateTime<Utc>) -> Self {
        self.end = Some(RecurrenceEnd::Until(until));
        self
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

impl Reminder {
    pub fn lookup(&self) -> ReminderLookup {
        ReminderIdentifierInput::from(self)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum ReadPathResult {
    Lists(Vec<ReminderList>),
    Reminders(Vec<Reminder>),
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, NaiveTime, Utc};

    use super::*;

    #[test]
    fn reminder_filter_builders_preserve_query_shape() {
        let from = DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let to = DateTime::parse_from_rfc3339("2026-04-18T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let filter = ReminderFilter::completed()
            .in_lists(["list-a", "list-b"])
            .between(Some(from), Some(to));

        assert_eq!(
            filter.list_ids,
            Some(vec!["list-a".to_string(), "list-b".to_string()])
        );
        assert_eq!(
            filter.kind,
            ReminderFilterKind::Completed {
                from: Some(from),
                to: Some(to),
            }
        );
    }

    #[test]
    fn create_reminder_input_builder_populates_extended_fields() {
        let completed_at = DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let input = CreateReminderInput::new("Ship API")
            .in_list("list-a")
            .with_notes("note")
            .with_url("https://char.com")
            .with_priority(ReminderPriority::High)
            .with_alarms(vec![])
            .with_recurrence_rules(vec![])
            .mark_completed()
            .with_completion_date(completed_at);

        assert_eq!(input.title, "Ship API");
        assert_eq!(input.list_id.as_deref(), Some("list-a"));
        assert_eq!(input.notes.as_deref(), Some("note"));
        assert_eq!(input.url.as_deref(), Some("https://char.com"));
        assert_eq!(input.priority, Some(ReminderPriority::High));
        assert_eq!(input.alarms, Some(Vec::new()));
        assert_eq!(input.recurrence_rules, Some(Vec::new()));
        assert_eq!(input.is_completed, Some(true));
        assert_eq!(input.completion_date, Some(completed_at));
    }

    #[test]
    fn reminder_identifier_builders_are_explicit() {
        let input =
            ReminderIdentifierInput::by_external_identifier("external-id").in_list("list-a");

        assert_eq!(input.external_identifier.as_deref(), Some("external-id"));
        assert_eq!(input.list_id.as_deref(), Some("list-a"));
        assert_eq!(input.calendar_item_identifier, None);
    }

    #[test]
    fn date_component_builders_cover_common_eventkit_shapes() {
        let date = NaiveDate::from_ymd_opt(2026, 4, 17).unwrap();
        let time = NaiveTime::from_hms_opt(9, 30, 15).unwrap();

        assert_eq!(
            DateComponents::all_day(date),
            DateComponents {
                date: Some(date),
                time: None,
                time_zone: None,
            }
        );
        assert_eq!(
            DateComponents::floating(date, time),
            DateComponents {
                date: Some(date),
                time: Some(time),
                time_zone: None,
            }
        );
        assert_eq!(
            DateComponents::zoned(date, time, "Asia/Seoul"),
            DateComponents {
                date: Some(date),
                time: Some(time),
                time_zone: Some("Asia/Seoul".into()),
            }
        );
    }

    #[test]
    fn recurrence_rule_builders_cover_common_schedules() {
        let until = DateTime::parse_from_rfc3339("2026-04-30T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let rule = RecurrenceRule::weekly(2)
            .on_days_of_week([
                RecurrenceDayOfWeek::every(Weekday::Monday),
                RecurrenceDayOfWeek::every(Weekday::Friday),
            ])
            .until(until);

        assert_eq!(rule.frequency, RecurrenceFrequency::Weekly);
        assert_eq!(rule.interval, 2);
        assert_eq!(
            rule.days_of_week,
            vec![
                RecurrenceDayOfWeek::every(Weekday::Monday),
                RecurrenceDayOfWeek::every(Weekday::Friday),
            ]
        );
        assert_eq!(rule.end, Some(RecurrenceEnd::Until(until)));
    }

    #[test]
    fn reminder_lookup_can_be_derived_from_reminder() {
        let reminder = Reminder {
            calendar_item_identifier: "calendar-id".into(),
            external_identifier: "external-id".into(),
            list: ReminderListRef {
                id: "list-a".into(),
                title: "Inbox".into(),
            },
            title: "Ship API".into(),
            notes: None,
            url: None,
            priority: ReminderPriority::None,
            is_completed: false,
            completion_date: None,
            start_date_components: None,
            due_date_components: None,
            creation_date: None,
            last_modified_date: None,
            has_alarms: false,
            has_recurrence_rules: false,
            alarms: Vec::new(),
            recurrence_rules: Vec::new(),
        };

        assert_eq!(
            reminder.lookup(),
            ReminderIdentifierInput::by_calendar_item_identifier("calendar-id")
        );
    }
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[cfg(feature = "specta")]
use specta::Type;

#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum EventShowAs {
    Free,
    Tentative,
    Busy,
    Oof,
    WorkingElsewhere,
    Unknown,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum Importance {
    Low,
    Normal,
    High,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum Sensitivity {
    Normal,
    Personal,
    Private,
    Confidential,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = outlook::EventType))]
#[serde(rename_all = "camelCase")]
pub enum EventType {
    SingleInstance,
    Occurrence,
    Exception,
    SeriesMaster,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum ResponseType {
    None,
    Organizer,
    TentativelyAccepted,
    Accepted,
    Declined,
    NotResponded,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum AttendeeType {
    Required,
    Optional,
    Resource,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum OnlineMeetingProviderType {
    TeamsForBusiness,
    SkypeForBusiness,
    SkypeForConsumer,
    Unknown,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum BodyType {
    Text,
    Html,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum CalendarColor {
    Auto,
    LightBlue,
    LightGreen,
    LightOrange,
    LightGray,
    LightYellow,
    LightTeal,
    LightPink,
    LightBrown,
    LightRed,
    MaxColor,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum DayOfWeek {
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum WeekIndex {
    First,
    Second,
    Third,
    Fourth,
    Last,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum RecurrencePatternType {
    Daily,
    Weekly,
    AbsoluteMonthly,
    RelativeMonthly,
    AbsoluteYearly,
    RelativeYearly,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum RecurrenceRangeType {
    EndDate,
    NoEnd,
    Numbered,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum LocationType {
    Default,
    ConferenceRoom,
    HomeAddress,
    BusinessAddress,
    GeoCoordinates,
    StreetAddress,
    Hotel,
    Restaurant,
    LocalBusiness,
    PostalAddress,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DateTimeTimeZone {
    pub date_time: String,
    #[serde(default)]
    pub time_zone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct EmailAddress {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ResponseStatus {
    #[serde(default)]
    pub response: Option<ResponseType>,
    #[serde(default)]
    pub time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = outlook::Attendee))]
#[serde(rename_all = "camelCase")]
pub struct Attendee {
    #[serde(default, rename = "type")]
    pub type_: Option<AttendeeType>,
    #[serde(default)]
    pub status: Option<ResponseStatus>,
    #[serde(default)]
    pub email_address: Option<EmailAddress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ItemBody {
    #[serde(default)]
    pub content_type: Option<BodyType>,
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PhysicalAddress {
    #[serde(default)]
    pub street: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub country_or_region: Option<String>,
    #[serde(default)]
    pub postal_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct OutlookGeoCoordinates {
    #[serde(default)]
    pub latitude: Option<f64>,
    #[serde(default)]
    pub longitude: Option<f64>,
    #[serde(default)]
    pub accuracy: Option<f64>,
    #[serde(default)]
    pub altitude: Option<f64>,
    #[serde(default)]
    pub altitude_accuracy: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Location {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub location_type: Option<LocationType>,
    #[serde(default)]
    pub unique_id: Option<String>,
    #[serde(default)]
    pub unique_id_type: Option<String>,
    #[serde(default)]
    pub address: Option<PhysicalAddress>,
    #[serde(default)]
    pub coordinates: Option<OutlookGeoCoordinates>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct OnlineMeetingInfo {
    #[serde(default)]
    pub join_url: Option<String>,
    #[serde(default)]
    pub conference_id: Option<String>,
    #[serde(default)]
    pub toll_number: Option<String>,
    #[serde(default)]
    pub toll_free_numbers: Option<Vec<String>>,
    #[serde(default)]
    pub quick_dial: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct RecurrencePattern {
    #[serde(default, rename = "type")]
    pub type_: Option<RecurrencePatternType>,
    #[serde(default)]
    pub interval: Option<i32>,
    #[serde(default)]
    pub month: Option<i32>,
    #[serde(default)]
    pub day_of_month: Option<i32>,
    #[serde(default)]
    pub days_of_week: Option<Vec<DayOfWeek>>,
    #[serde(default)]
    pub first_day_of_week: Option<DayOfWeek>,
    #[serde(default)]
    pub index: Option<WeekIndex>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct RecurrenceRange {
    #[serde(default, rename = "type")]
    pub type_: Option<RecurrenceRangeType>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub recurrence_time_zone: Option<String>,
    #[serde(default)]
    pub number_of_occurrences: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PatternedRecurrence {
    #[serde(default)]
    pub pattern: Option<RecurrencePattern>,
    #[serde(default)]
    pub range: Option<RecurrenceRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Calendar {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub color: Option<CalendarColor>,
    #[serde(default)]
    pub hex_color: Option<String>,
    #[serde(default)]
    pub is_default_calendar: Option<bool>,
    #[serde(default)]
    pub change_key: Option<String>,
    #[serde(default)]
    pub can_share: Option<bool>,
    #[serde(default)]
    pub can_view_private_items: Option<bool>,
    #[serde(default)]
    pub can_edit: Option<bool>,
    #[serde(default)]
    pub is_removable: Option<bool>,
    #[serde(default)]
    pub is_tallying_responses: Option<bool>,
    #[serde(default)]
    pub owner: Option<EmailAddress>,
    #[serde(default)]
    pub default_online_meeting_provider: Option<OnlineMeetingProviderType>,
    #[serde(default)]
    pub allowed_online_meeting_providers: Option<Vec<OnlineMeetingProviderType>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = outlook::Event))]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    #[serde(default, rename = "iCalUId")]
    pub ical_uid: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub body: Option<ItemBody>,
    #[serde(default)]
    pub body_preview: Option<String>,
    #[serde(default)]
    pub start: Option<DateTimeTimeZone>,
    #[serde(default)]
    pub end: Option<DateTimeTimeZone>,
    #[serde(default)]
    pub location: Option<Location>,
    #[serde(default)]
    pub locations: Option<Vec<Location>>,
    #[serde(default)]
    pub attendees: Option<Vec<Attendee>>,
    #[serde(default)]
    pub organizer: Option<Recipient>,
    #[serde(default)]
    pub is_all_day: Option<bool>,
    #[serde(default)]
    pub is_cancelled: Option<bool>,
    #[serde(default)]
    pub is_organizer: Option<bool>,
    #[serde(default)]
    pub is_draft: Option<bool>,
    #[serde(default)]
    pub is_online_meeting: Option<bool>,
    #[serde(default)]
    pub is_reminder_on: Option<bool>,
    #[serde(default)]
    pub response_requested: Option<bool>,
    #[serde(default)]
    pub has_attachments: Option<bool>,
    #[serde(default)]
    pub importance: Option<Importance>,
    #[serde(default)]
    pub sensitivity: Option<Sensitivity>,
    #[serde(default)]
    pub show_as: Option<EventShowAs>,
    #[serde(default, rename = "type")]
    pub type_: Option<EventType>,
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    pub web_link: Option<String>,
    #[serde(default)]
    pub online_meeting_url: Option<String>,
    #[serde(default)]
    pub online_meeting_provider: Option<OnlineMeetingProviderType>,
    #[serde(default)]
    pub online_meeting: Option<OnlineMeetingInfo>,
    #[serde(default)]
    pub recurrence: Option<PatternedRecurrence>,
    #[serde(default)]
    pub series_master_id: Option<String>,
    #[serde(default)]
    pub response_status: Option<ResponseStatus>,
    #[serde(default)]
    pub reminder_minutes_before_start: Option<i32>,
    #[serde(default)]
    pub allow_new_time_proposals: Option<bool>,
    #[serde(default)]
    pub hide_attendees: Option<bool>,
    #[serde(default)]
    pub change_key: Option<String>,
    #[serde(default)]
    pub created_date_time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_modified_date_time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub original_start_time_zone: Option<String>,
    #[serde(default)]
    pub original_end_time_zone: Option<String>,
    #[serde(default)]
    pub transaction_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Recipient {
    #[serde(default)]
    pub email_address: Option<EmailAddress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = outlook::ListCalendarsResponse))]
#[serde(rename_all = "camelCase")]
pub struct ListCalendarsResponse {
    #[serde(default, rename = "@odata.context")]
    pub odata_context: Option<String>,
    #[serde(default, rename = "@odata.nextLink")]
    pub odata_next_link: Option<String>,
    #[serde(default)]
    pub value: Vec<Calendar>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = outlook::ListEventsResponse))]
#[serde(rename_all = "camelCase")]
pub struct ListEventsResponse {
    #[serde(default, rename = "@odata.context")]
    pub odata_context: Option<String>,
    #[serde(default, rename = "@odata.nextLink")]
    pub odata_next_link: Option<String>,
    #[serde(default)]
    pub value: Vec<Event>,
}

#[derive(Default)]
pub struct ListEventsRequest {
    pub calendar_id: String,
    pub start_date_time: Option<DateTime<Utc>>,
    pub end_date_time: Option<DateTime<Utc>>,
    pub top: Option<u32>,
    pub skip: Option<u32>,
    pub filter: Option<String>,
    pub select: Option<Vec<String>>,
    pub order_by: Option<String>,
}

pub struct CreateEventRequest {
    pub calendar_id: String,
    pub event: CreateEventBody,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = outlook::CreateEventBody))]
#[serde(rename_all = "camelCase")]
pub struct CreateEventBody {
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub start: DateTimeTimeZone,
    #[serde(default)]
    pub end: DateTimeTimeZone,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<ItemBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attendees: Option<Vec<Attendee>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_all_day: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importance: Option<Importance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<Sensitivity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_as: Option<EventShowAs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<PatternedRecurrence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_online_meeting: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_meeting_provider: Option<OnlineMeetingProviderType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_reminder_on: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reminder_minutes_before_start: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_requested: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_new_time_proposals: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_attendees: Option<bool>,
}

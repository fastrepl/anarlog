use std::collections::HashMap;

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[cfg(feature = "specta")]
use specta::Type;

#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

// === Enums (response-side with forward-compatible Unknown fallback) ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum EventStatus {
    Confirmed,
    Tentative,
    Cancelled,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum AccessRole {
    FreeBusyReader,
    Reader,
    Writer,
    Owner,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum AttendeeResponseStatus {
    NeedsAction,
    Declined,
    Tentative,
    Accepted,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = google::EventType))]
#[serde(rename_all = "camelCase")]
pub enum EventType {
    Default,
    Birthday,
    FocusTime,
    FromGmail,
    OutOfOffice,
    WorkingLocation,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum Transparency {
    Opaque,
    Transparent,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum Visibility {
    Default,
    Public,
    Private,
    Confidential,
    #[serde(other)]
    Unknown,
}

// === Enums (request-side, no Unknown needed) ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum EventOrderBy {
    StartTime,
    Updated,
}

// === Typed enums replacing free-form strings ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum ConferenceSolutionType {
    AddOn,
    HangoutsMeet,
    EventNamedHangout,
    EventHangout,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum EntryPointType {
    Video,
    Phone,
    Sip,
    More,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum ReminderMethod {
    Email,
    Popup,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum NotificationMethod {
    Email,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum NotificationType {
    EventCreation,
    EventChange,
    EventCancellation,
    EventResponse,
    Agenda,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum ConferenceCreateStatusCode {
    Pending,
    Success,
    Failure,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum WorkingLocationType {
    HomeOffice,
    OfficeLocation,
    CustomLocation,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum AutoDeclineMode {
    DeclineNone,
    DeclineAllConflictingInvitations,
    DeclineOnlyNewConflictingInvitations,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum ChatStatus {
    Available,
    DoNotDisturb,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum BirthdayPropertyType {
    Birthday,
    Anniversary,
    #[serde(rename = "self")]
    CalendarOwner,
    Other,
    Custom,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum GadgetDisplay {
    Chip,
    Icon,
    #[serde(other)]
    Unknown,
}

// === Conference nested structs ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConferenceData {
    #[serde(default)]
    pub conference_id: Option<String>,
    #[serde(default)]
    pub conference_solution: Option<ConferenceSolution>,
    #[serde(default)]
    pub entry_points: Option<Vec<EntryPoint>>,
    #[serde(default)]
    pub create_request: Option<ConferenceCreateRequest>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConferenceSolution {
    #[serde(default)]
    pub key: Option<ConferenceSolutionKey>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub icon_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConferenceSolutionKey {
    #[serde(rename = "type")]
    pub type_: ConferenceSolutionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConferenceCreateRequest {
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub conference_solution_key: Option<ConferenceSolutionKey>,
    #[serde(default)]
    pub status: Option<ConferenceCreateRequestStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConferenceCreateRequestStatus {
    pub status_code: ConferenceCreateStatusCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct EntryPoint {
    pub entry_point_type: EntryPointType,
    pub uri: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub pin: Option<String>,
    #[serde(default)]
    pub access_code: Option<String>,
    #[serde(default)]
    pub meeting_code: Option<String>,
    #[serde(default)]
    pub passcode: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConferenceProperties {
    #[serde(default)]
    pub allowed_conference_solution_types: Option<Vec<ConferenceSolutionType>>,
}

// === Reminder / notification nested structs ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Reminders {
    #[serde(default)]
    pub use_default: Option<bool>,
    #[serde(default)]
    pub overrides: Option<Vec<Reminder>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub method: ReminderMethod,
    pub minutes: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    #[serde(default)]
    pub notifications: Option<Vec<CalendarNotification>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CalendarNotification {
    pub method: NotificationMethod,
    #[serde(rename = "type")]
    pub type_: NotificationType,
}

// === Event-type-specific property structs ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Gadget {
    #[serde(default, rename = "type")]
    pub type_: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default)]
    pub icon_link: Option<String>,
    #[serde(default)]
    pub width: Option<i32>,
    #[serde(default)]
    pub height: Option<i32>,
    #[serde(default)]
    pub display: Option<GadgetDisplay>,
    #[serde(default)]
    pub preferences: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct WorkingLocationProperties {
    #[serde(default, rename = "type")]
    pub type_: Option<WorkingLocationType>,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(skip))]
    pub home_office: Option<serde_json::Value>,
    #[serde(default)]
    pub custom_location: Option<CustomLocation>,
    #[serde(default)]
    pub office_location: Option<OfficeLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CustomLocation {
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct OfficeLocation {
    #[serde(default)]
    pub building_id: Option<String>,
    #[serde(default)]
    pub floor_id: Option<String>,
    #[serde(default)]
    pub floor_section_id: Option<String>,
    #[serde(default)]
    pub desk_id: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct OutOfOfficeProperties {
    #[serde(default)]
    pub auto_decline_mode: Option<AutoDeclineMode>,
    #[serde(default)]
    pub decline_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct FocusTimeProperties {
    #[serde(default)]
    pub auto_decline_mode: Option<AutoDeclineMode>,
    #[serde(default)]
    pub decline_message: Option<String>,
    #[serde(default)]
    pub chat_status: Option<ChatStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BirthdayProperties {
    #[serde(default)]
    pub contact: Option<String>,
    #[serde(default, rename = "type")]
    pub type_: Option<BirthdayPropertyType>,
    #[serde(default)]
    pub custom_type_name: Option<String>,
}

// === Other nested structs ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct EventSource {
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExtendedProperties {
    #[serde(default)]
    pub private: Option<HashMap<String, String>>,
    #[serde(default)]
    pub shared: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct EventAttachment {
    #[serde(default)]
    pub file_url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub icon_link: Option<String>,
    #[serde(default)]
    pub file_id: Option<String>,
}

// === CalendarListEntry resource ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CalendarListEntry {
    pub id: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub time_zone: Option<String>,
    #[serde(default)]
    pub summary_override: Option<String>,
    #[serde(default)]
    pub color_id: Option<String>,
    #[serde(default)]
    pub background_color: Option<String>,
    #[serde(default)]
    pub foreground_color: Option<String>,
    #[serde(default)]
    pub hidden: Option<bool>,
    #[serde(default)]
    pub selected: Option<bool>,
    #[serde(default)]
    pub primary: Option<bool>,
    #[serde(default)]
    pub deleted: Option<bool>,
    #[serde(default)]
    pub access_role: Option<AccessRole>,
    #[serde(default)]
    pub data_owner: Option<String>,
    #[serde(default)]
    pub default_reminders: Option<Vec<Reminder>>,
    #[serde(default)]
    pub notification_settings: Option<NotificationSettings>,
    #[serde(default)]
    pub conference_properties: Option<ConferenceProperties>,
    #[serde(default)]
    pub auto_accept_invitations: Option<bool>,
}

// === Event resource ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = google::Event))]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub status: Option<EventStatus>,
    #[serde(default)]
    pub html_link: Option<String>,
    #[serde(default)]
    pub created: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub color_id: Option<String>,
    #[serde(default)]
    pub creator: Option<EventPerson>,
    #[serde(default)]
    pub organizer: Option<EventPerson>,
    #[serde(default)]
    pub start: Option<EventDateTime>,
    #[serde(default)]
    pub end: Option<EventDateTime>,
    #[serde(default)]
    pub end_time_unspecified: Option<bool>,
    #[serde(default)]
    pub recurrence: Option<Vec<String>>,
    #[serde(default)]
    pub recurring_event_id: Option<String>,
    #[serde(default)]
    pub original_start_time: Option<EventDateTime>,
    #[serde(default)]
    pub transparency: Option<Transparency>,
    #[serde(default)]
    pub visibility: Option<Visibility>,
    #[serde(default, rename = "iCalUID")]
    pub ical_uid: Option<String>,
    #[serde(default)]
    pub sequence: Option<i32>,
    #[serde(default)]
    pub attendees: Option<Vec<Attendee>>,
    #[serde(default)]
    pub attendees_omitted: Option<bool>,
    #[serde(default)]
    pub extended_properties: Option<ExtendedProperties>,
    #[serde(default)]
    pub hangout_link: Option<String>,
    #[serde(default)]
    pub conference_data: Option<ConferenceData>,
    #[serde(default)]
    pub gadget: Option<Gadget>,
    #[serde(default)]
    pub anyone_can_add_self: Option<bool>,
    #[serde(default)]
    pub guests_can_invite_others: Option<bool>,
    #[serde(default)]
    pub guests_can_modify: Option<bool>,
    #[serde(default)]
    pub guests_can_see_other_guests: Option<bool>,
    #[serde(default)]
    pub private_copy: Option<bool>,
    #[serde(default)]
    pub locked: Option<bool>,
    #[serde(default)]
    pub reminders: Option<Reminders>,
    #[serde(default)]
    pub source: Option<EventSource>,
    #[serde(default)]
    pub working_location_properties: Option<WorkingLocationProperties>,
    #[serde(default)]
    pub out_of_office_properties: Option<OutOfOfficeProperties>,
    #[serde(default)]
    pub focus_time_properties: Option<FocusTimeProperties>,
    #[serde(default)]
    pub attachments: Option<Vec<EventAttachment>>,
    #[serde(default)]
    pub birthday_properties: Option<BirthdayProperties>,
    #[serde(default)]
    pub event_type: Option<EventType>,
}

// === Sub-resource structs ===

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct EventDateTime {
    #[serde(default)]
    pub date: Option<NaiveDate>,
    #[serde(default)]
    pub date_time: Option<DateTime<FixedOffset>>,
    #[serde(default)]
    pub time_zone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct EventPerson {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default, rename = "self")]
    pub is_self: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = google::Attendee))]
#[serde(rename_all = "camelCase")]
pub struct Attendee {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub organizer: Option<bool>,
    #[serde(default, rename = "self")]
    pub is_self: Option<bool>,
    #[serde(default)]
    pub resource: Option<bool>,
    #[serde(default)]
    pub optional: Option<bool>,
    #[serde(default)]
    pub response_status: Option<AttendeeResponseStatus>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub additional_guests: Option<i32>,
}

// === Request / Response types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = google::ListCalendarsResponse))]
#[serde(rename_all = "camelCase")]
pub struct ListCalendarsResponse {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub next_page_token: Option<String>,
    #[serde(default)]
    pub next_sync_token: Option<String>,
    #[serde(default)]
    pub items: Vec<CalendarListEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = google::ListEventsResponse))]
#[serde(rename_all = "camelCase")]
pub struct ListEventsResponse {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub time_zone: Option<String>,
    #[serde(default)]
    pub access_role: Option<AccessRole>,
    #[serde(default)]
    pub default_reminders: Option<Vec<Reminder>>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
    #[serde(default)]
    pub next_page_token: Option<String>,
    #[serde(default)]
    pub next_sync_token: Option<String>,
    #[serde(default)]
    pub items: Vec<Event>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct ListEventsRequest {
    pub calendar_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_min: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_max: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub single_events: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_by: Option<EventOrderBy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_deleted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_hidden_invitations: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_min: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub i_cal_uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_types: Option<Vec<EventType>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct CreateEventRequest {
    pub calendar_id: String,
    pub event: CreateEventBody,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(as = google::CreateEventBody))]
#[serde(rename_all = "camelCase")]
pub struct CreateEventBody {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub start: EventDateTime,
    #[serde(default)]
    pub end: EventDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attendees: Option<Vec<Attendee>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transparency: Option<Transparency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conference_data: Option<ConferenceData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reminders: Option<Reminders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guests_can_invite_others: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guests_can_modify: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guests_can_see_other_guests: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<EventSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extended_properties: Option<ExtendedProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<EventType>,
}

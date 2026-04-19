use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use hypr_calendar_interface::CalendarProviderType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionKey {
    pub provider: CalendarProviderType,
    pub connection_id: String,
}

impl ConnectionKey {
    pub fn new(provider: CalendarProviderType, connection_id: impl Into<String>) -> Self {
        Self {
            provider,
            connection_id: connection_id.into(),
        }
    }
}

impl PartialOrd for ConnectionKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ConnectionKey {
    fn cmp(&self, other: &Self) -> Ordering {
        provider_tag(self.provider)
            .cmp(provider_tag(other.provider))
            .then_with(|| self.connection_id.cmp(&other.connection_id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarKey {
    pub provider: CalendarProviderType,
    pub connection_id: String,
    pub tracking_id: String,
}

impl PartialOrd for CalendarKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CalendarKey {
    fn cmp(&self, other: &Self) -> Ordering {
        provider_tag(self.provider)
            .cmp(provider_tag(other.provider))
            .then_with(|| self.connection_id.cmp(&other.connection_id))
            .then_with(|| self.tracking_id.cmp(&other.tracking_id))
    }
}

impl CalendarKey {
    pub fn new(
        provider: CalendarProviderType,
        connection_id: impl Into<String>,
        tracking_id: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            connection_id: connection_id.into(),
            tracking_id: tracking_id.into(),
        }
    }

    pub fn connection_key(&self) -> ConnectionKey {
        ConnectionKey {
            provider: self.provider,
            connection_id: self.connection_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarPayload {
    pub name: String,
    pub source: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingCalendar {
    pub key: CalendarKey,
    pub payload: CalendarPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingParticipant {
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_organizer: bool,
    pub is_current_user: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EventPayload {
    pub title: Option<String>,
    pub location: Option<String>,
    pub meeting_link: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingEvent {
    pub calendar_key: CalendarKey,
    pub tracking_id_event: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub recurrence_series_id: Option<String>,
    pub has_recurrence_rules: bool,
    pub is_all_day: bool,
    pub participants: Vec<IncomingParticipant>,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncRange {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

pub trait PersistedCalendar {
    fn id(&self) -> &str;
    fn key(&self) -> CalendarKey;
    fn enabled(&self) -> bool;
}

pub trait PersistedEvent {
    fn id(&self) -> &str;
    fn tracking_id_event(&self) -> Option<&str>;
    fn calendar_id(&self) -> &str;
    fn started_at(&self) -> &str;
    fn ended_at(&self) -> Option<&str>;
}

fn provider_tag(provider: CalendarProviderType) -> &'static str {
    match provider {
        CalendarProviderType::Apple => "apple",
        CalendarProviderType::Google => "google",
        CalendarProviderType::Outlook => "outlook",
    }
}

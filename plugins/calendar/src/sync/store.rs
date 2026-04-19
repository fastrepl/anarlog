use hypr_calendar_interface::CalendarProviderType;

const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarRecord {
    pub user_id: String,
    pub created_at: String,
    pub tracking_id_calendar: String,
    pub name: String,
    pub enabled: bool,
    pub provider: CalendarProviderType,
    pub source: String,
    pub color: String,
    pub connection_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantRecord {
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_organizer: bool,
    pub is_current_user: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    pub user_id: String,
    pub created_at: String,
    pub tracking_id_event: Option<String>,
    pub calendar_id: String,
    pub title: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub location: Option<String>,
    pub meeting_link: Option<String>,
    pub description: Option<String>,
    pub note: Option<String>,
    pub recurrence_series_id: Option<String>,
    pub has_recurrence_rules: bool,
    pub is_all_day: bool,
    pub provider: CalendarProviderType,
    pub participants: Vec<ParticipantRecord>,
}

pub(crate) fn default_user_id() -> String {
    DEFAULT_USER_ID.to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCalendarRecord {
    pub id: String,
    pub record: CalendarRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEventRecord {
    pub id: String,
    pub record: EventRecord,
}

impl hypr_calendar_sync::PersistedCalendar for StoredCalendarRecord {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> hypr_calendar_sync::CalendarKey {
        hypr_calendar_sync::CalendarKey::new(
            self.record.provider,
            self.record.connection_id.clone(),
            self.record.tracking_id_calendar.clone(),
        )
    }

    fn enabled(&self) -> bool {
        self.record.enabled
    }
}

impl hypr_calendar_sync::PersistedEvent for StoredEventRecord {
    fn id(&self) -> &str {
        &self.id
    }

    fn tracking_id_event(&self) -> Option<&str> {
        self.record.tracking_id_event.as_deref()
    }

    fn calendar_id(&self) -> &str {
        &self.record.calendar_id
    }

    fn started_at(&self) -> &str {
        &self.record.started_at
    }

    fn ended_at(&self) -> Option<&str> {
        self.record.ended_at.as_deref()
    }
}

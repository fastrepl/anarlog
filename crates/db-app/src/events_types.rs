#[cfg_attr(feature = "cli", derive(serde::Serialize))]
pub struct EventRow {
    pub id: String,
    pub user_id: String,
    pub calendar_id: String,
    pub tracking_id: String,
    pub title: String,
    pub started_at: String,
    pub ended_at: String,
    pub location: String,
    pub meeting_link: String,
    pub description: String,
    pub note: String,
    pub recurrence_series_id: String,
    pub has_recurrence_rules: bool,
    pub is_all_day: bool,
    pub participants_json: String,
    pub raw_json: String,
    pub created_at: String,
}

pub struct EventParticipantRow {
    pub id: String,
    pub event_id: String,
    pub human_id: Option<String>,
    pub email: String,
    pub name: String,
    pub is_organizer: bool,
    pub is_current_user: bool,
    pub user_id: String,
}

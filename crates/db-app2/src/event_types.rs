#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Event {
    pub id: String,
    pub tracking_id_event: String,
    pub calendar_id: String,
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
    pub provider: String,
    pub participants_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct UpsertEvent<'a> {
    pub id: &'a str,
    pub tracking_id_event: &'a str,
    pub calendar_id: &'a str,
    pub title: &'a str,
    pub started_at: &'a str,
    pub ended_at: &'a str,
    pub location: &'a str,
    pub meeting_link: &'a str,
    pub description: &'a str,
    pub note: &'a str,
    pub recurrence_series_id: &'a str,
    pub has_recurrence_rules: bool,
    pub is_all_day: bool,
    pub provider: &'a str,
    pub participants_json: Option<&'a str>,
}

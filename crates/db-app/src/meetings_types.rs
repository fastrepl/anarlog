#[cfg_attr(feature = "cli", derive(serde::Serialize))]
pub struct MeetingRow {
    pub id: String,
    pub created_at: String,
    pub title: Option<String>,
    pub user_id: String,
    pub visibility: String,
    pub folder_id: Option<String>,
    pub event_id: Option<String>,
}

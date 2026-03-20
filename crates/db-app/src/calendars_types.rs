#[derive(Clone)]
pub struct CalendarRow {
    pub id: String,
    pub provider: String,
    pub connection_id: String,
    pub tracking_id: String,
    pub name: String,
    pub color: String,
    pub source: String,
    pub enabled: bool,
    pub created_at: String,
    pub user_id: String,
    pub raw_json: String,
}

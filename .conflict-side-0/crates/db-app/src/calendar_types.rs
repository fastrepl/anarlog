#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct CalendarRow {
    pub id: String,
    pub tracking_id_calendar: String,
    pub name: String,
    pub enabled: bool,
    pub provider: String,
    pub source: String,
    pub color: String,
    pub connection_id: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct UpsertCalendar<'a> {
    pub id: &'a str,
    pub tracking_id_calendar: &'a str,
    pub name: &'a str,
    pub enabled: bool,
    pub provider: &'a str,
    pub source: &'a str,
    pub color: &'a str,
    pub connection_id: &'a str,
}

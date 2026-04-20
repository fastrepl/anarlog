#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Calendar {
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

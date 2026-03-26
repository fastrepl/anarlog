use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct StoredCalendar {
    pub id: String,
    pub provider: String,
    pub connection_id: String,
    pub tracking_id: String,
    pub name: String,
    pub color: String,
    pub source: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct StoredEvent {
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

#[derive(Debug)]
pub struct UpsertEvent {
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
}

#[derive(Debug)]
pub struct UpsertCalendar {
    pub id: String,
    pub provider: String,
    pub connection_id: String,
    pub tracking_id: String,
    pub name: String,
    pub color: String,
    pub source: String,
    pub enabled: bool,
}

#[async_trait]
pub trait CalendarStorage: Send + Sync {
    async fn list_all_calendars_for_provider(
        &self,
        provider: &str,
    ) -> Result<Vec<StoredCalendar>, Box<dyn std::error::Error + Send + Sync>>;

    async fn list_enabled_calendars(
        &self,
        provider: &str,
        connection_id: &str,
    ) -> Result<Vec<StoredCalendar>, Box<dyn std::error::Error + Send + Sync>>;

    async fn list_events_for_calendars(
        &self,
        calendar_ids: &[String],
        from: &str,
        to: &str,
    ) -> Result<Vec<StoredEvent>, Box<dyn std::error::Error + Send + Sync>>;

    async fn upsert_event(
        &self,
        event: UpsertEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    async fn delete_event(&self, id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    async fn upsert_calendar(
        &self,
        cal: UpsertCalendar,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    async fn delete_calendar(
        &self,
        id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    async fn delete_events_by_calendar(
        &self,
        calendar_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

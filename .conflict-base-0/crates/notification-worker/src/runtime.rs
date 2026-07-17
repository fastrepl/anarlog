#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[cfg_attr(feature = "tauri-event", derive(tauri_specta::Event))]
#[serde(tag = "type")]
pub enum NotificationWorkerEvent {
    #[serde(rename = "upcomingEvent")]
    UpcomingEvent {
        event_id: String,
        title: String,
        started_at: String,
        minutes_until: i64,
        participants: Vec<String>,
    },
}

pub trait NotificationWorkerRuntime: Send + Sync + 'static {
    fn emit(&self, event: NotificationWorkerEvent);
}

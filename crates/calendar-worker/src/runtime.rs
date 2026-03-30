#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[cfg_attr(feature = "tauri-event", derive(tauri_specta::Event))]
#[serde(tag = "type")]
pub enum NotificationWorkerEvent {
    #[serde(rename = "eventStarted")]
    EventStarted {
        event_id: String,
        title: String,
        started_at: String,
        participants: Vec<String>,
    },
}

pub trait NotificationWorkerRuntime: Send + Sync + 'static {
    fn emit(&self, event: NotificationWorkerEvent);
}

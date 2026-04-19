#[derive(serde::Serialize, Clone, specta::Type, tauri_specta::Event)]
pub struct CalendarChangedEvent;

#[derive(serde::Serialize, Clone, specta::Type, tauri_specta::Event)]
#[serde(tag = "type")]
pub enum CalendarSyncEvent {
    #[serde(rename = "statusChanged")]
    StatusChanged {
        status: hypr_calendar_sync::SyncStatus,
    },
    #[serde(rename = "syncStarted")]
    SyncStarted,
    #[serde(rename = "syncFinished")]
    SyncFinished { data_changed: bool },
    #[serde(rename = "syncFailed")]
    SyncFailed { error: String },
}

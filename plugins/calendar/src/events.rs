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
    SyncStarted {
        reasons: Vec<hypr_calendar_sync::SyncReason>,
    },
    #[serde(rename = "syncFinished")]
    SyncFinished {
        reasons: Vec<hypr_calendar_sync::SyncReason>,
        data_changed: bool,
    },
    #[serde(rename = "syncFailed")]
    SyncFailed {
        reasons: Vec<hypr_calendar_sync::SyncReason>,
        error: String,
    },
}

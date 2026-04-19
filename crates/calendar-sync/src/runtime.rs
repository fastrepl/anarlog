#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum SyncReason {
    Startup,
    Interval,
    AppleCalendarChanged,
    Manual,
    Deeplink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Idle,
    Scheduled,
    Running,
}

#[derive(Debug, Clone)]
pub enum CalendarSyncWorkerEvent {
    StatusChanged {
        status: SyncStatus,
    },
    SyncStarted {
        reasons: Vec<SyncReason>,
    },
    SyncFinished {
        reasons: Vec<SyncReason>,
        data_changed: bool,
    },
    SyncFailed {
        reasons: Vec<SyncReason>,
        error: String,
    },
}

pub trait CalendarSyncRuntime: Send + Sync + 'static {
    fn emit(&self, event: CalendarSyncWorkerEvent);
}
